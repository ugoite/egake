//! A local-file implementation of the JSON Resource Contract for CSV and Parquet data.

mod parquet_backend;

pub mod config;

use std::{
    cmp::Ordering,
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use csv::{ReaderBuilder, StringRecord, WriterBuilder};
use egake_resource::{
    Capability, FieldSchema, FieldType, JsonResourceProvider, ListQuery, ResourceError,
    ResourceErrorKind, ResourcePage, ResourceProvider, ResourceResult, ResourceSchema,
    SortDirection, apply_merge_patch, require_object_patch,
};
use serde_json::{Map, Value};

pub use config::{DEFAULT_RESOURCE_KEY, DataConfigError, DataFormat, DataResourceConfig};
use parquet_backend::ParquetBackend;

static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
static TEMP_COUNTER: OnceLock<std::sync::atomic::AtomicU64> = OnceLock::new();

/// A file-backed provider for small local CSV datasets.
pub(crate) struct CsvBackend {
    path: PathBuf,
    key: String,
    headers: Vec<String>,
    schema: ResourceSchema,
    write_lock: Arc<Mutex<()>>,
    writable: bool,
    backup_count: u8,
}

impl std::fmt::Debug for CsvBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CsvBackend")
            .field("path", &self.path)
            .field("key", &self.key)
            .field("headers", &self.headers)
            .field("writable", &self.writable)
            .field("backup_count", &self.backup_count)
            .finish_non_exhaustive()
    }
}

impl CsvBackend {
    /// Opens and validates a CSV backend.
    pub fn new(config: DataResourceConfig) -> ResourceResult<Self> {
        config.validate().map_err(config_error)?;
        let path = fs::canonicalize(config.path()).map_err(|error| io_error("open CSV", error))?;
        let metadata = fs::metadata(&path).map_err(|error| io_error("inspect CSV", error))?;
        if !metadata.is_file() {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "data path must refer to a regular file",
            ));
        }

        let (headers, records) = read_rows(&path)?;
        let key_index = headers.iter().position(|header| header == config.key());
        if config.writable() && key_index.is_none() {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "writable CSV must contain its configured primary-key column",
            )
            .with_field("key", config.key()));
        }
        if let Some(key_index) = key_index {
            validate_unique_keys(&records, key_index, config.key())?;
        }
        if let Some(configured_schema) = config.schema() {
            for field in &configured_schema.fields {
                if !headers.iter().any(|header| header == &field.name) {
                    return Err(ResourceError::new(
                        ResourceErrorKind::Validation,
                        "CSV is missing a schema column",
                    )
                    .with_field(&field.name, "column does not exist"));
                }
            }
        }

        let name = config
            .name()
            .map(str::to_owned)
            .or_else(|| path.file_stem().and_then(|stem| stem.to_str()).map(str::to_owned))
            .unwrap_or_else(|| "resource".to_owned());
        let configured_schema = config.schema().cloned();
        let mut schema = ResourceSchema::new(name);
        for header in &headers {
            let field = configured_schema
                .as_ref()
                .and_then(|configured| configured.fields.iter().find(|field| field.name == *header))
                .cloned()
                .unwrap_or_else(|| FieldSchema::new(header, FieldType::Text));
            schema.push_field(if header == config.key() { field.required() } else { field });
        }
        schema.grant(Capability::Schema);
        schema.grant(Capability::List);
        if key_index.is_some() {
            schema.grant(Capability::Get);
            if config.writable() {
                schema.grant(Capability::Create);
                schema.grant(Capability::Update);
                schema.grant(Capability::Delete);
            }
        }

        Ok(Self {
            path: path.clone(),
            key: config.key().to_owned(),
            headers,
            schema,
            write_lock: path_lock(&path),
            writable: config.writable(),
            backup_count: config.backup_count(),
        })
    }

    /// Returns the canonical file path used by this provider.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn key_index(&self) -> ResourceResult<usize> {
        self.headers.iter().position(|header| header == &self.key).ok_or_else(|| {
            ResourceError::new(
                ResourceErrorKind::CapabilityDenied,
                "data resource has no configured primary-key column",
            )
        })
    }

    fn ensure_writable(&self) -> ResourceResult<()> {
        if self.writable {
            Ok(())
        } else {
            Err(ResourceError::new(
                ResourceErrorKind::CapabilityDenied,
                "data resource is read-only",
            ))
        }
    }

    fn read_values(&self) -> ResourceResult<Vec<Map<String, Value>>> {
        let (headers, records) = read_rows(&self.path)?;
        if headers != self.headers {
            return Err(ResourceError::new(
                ResourceErrorKind::Conflict,
                "data headers changed while the provider was open",
            ));
        }
        let key_index = self.headers.iter().position(|header| header == &self.key);
        if let Some(key_index) = key_index {
            validate_unique_keys(&records, key_index, &self.key)?;
        }
        Ok(records.into_iter().map(|record| record_to_object(&self.headers, &record)).collect())
    }

    fn write_values_locked(&self, values: &[Map<String, Value>]) -> ResourceResult<()> {
        let (headers, current_records) = read_rows(&self.path)?;
        if headers != self.headers {
            return Err(ResourceError::new(
                ResourceErrorKind::Conflict,
                "data headers changed while the provider was open",
            ));
        }
        if let Some(key_index) = self.headers.iter().position(|header| header == &self.key) {
            validate_unique_keys(&current_records, key_index, &self.key)?;
            validate_unique_value_keys(values, &self.key)?;
        }
        let temp_path = temporary_path(&self.path);
        let permissions = fs::metadata(&self.path)
            .map_err(|error| io_error("inspect data permissions", error))?
            .permissions();
        let write_result = write_csv(&temp_path, &self.headers, values)
            .and_then(|()| fs::set_permissions(&temp_path, permissions))
            .and_then(|()| retain_backups(&self.path, self.backup_count))
            .and_then(|()| fs::rename(&temp_path, &self.path))
            .and_then(|()| sync_parent_directory(&self.path));
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result.map_err(|error| io_error("write CSV", error))
    }
}

impl JsonResourceProvider for CsvBackend {
    fn schema(&self) -> ResourceResult<ResourceSchema> {
        Ok(self.schema.clone())
    }

    fn list(&self, query: &ListQuery) -> ResourceResult<ResourcePage<Value>> {
        let mut values: Vec<Value> = self.read_values()?.into_iter().map(Value::Object).collect();
        for sort in &query.sort {
            if !self.headers.iter().any(|header| header == &sort.field) {
                return Err(ResourceError::new(
                    ResourceErrorKind::Validation,
                    "unknown sort field",
                )
                .with_field("sort", &sort.field));
            }
        }
        values.retain(|value| match &query.search {
            Some(search) => {
                let needle = search.to_lowercase();
                value.as_object().is_some_and(|object| {
                    object.values().any(|field| {
                        field.as_str().is_some_and(|text| text.to_lowercase().contains(&needle))
                    })
                })
            }
            None => true,
        });
        if !query.sort.is_empty() {
            values.sort_by(|left, right| compare_values(left, right, &query.sort));
        }

        let total = values.len() as u64;
        let start = usize::try_from(query.offset).unwrap_or(usize::MAX).min(values.len());
        let end = start.saturating_add(query.limit as usize).min(values.len());
        let items = values.into_iter().skip(start).take(end.saturating_sub(start)).collect();
        Ok(ResourcePage::new(items, total, query.offset, query.limit))
    }

    fn get(&self, id: &str) -> ResourceResult<Option<Value>> {
        let key = self.key_index()?;
        Ok(self
            .read_values()?
            .into_iter()
            .find(|value| value.get(&self.headers[key]).and_then(Value::as_str) == Some(id))
            .map(Value::Object))
    }

    fn create(&self, value: Value) -> ResourceResult<Value> {
        self.ensure_writable()?;
        let key_index = self.key_index()?;
        let object = object_value(value, "created value")?;
        validate_columns(&self.headers, &object)?;
        let key = required_key(&object, &self.key)?;
        let _guard = self.write_lock.lock().map_err(|_| {
            ResourceError::new(ResourceErrorKind::Internal, "data write lock is poisoned")
        })?;
        let mut values = self.read_values()?;
        if values
            .iter()
            .any(|existing| existing.get(&self.key).and_then(Value::as_str) == Some(key))
        {
            return Err(ResourceError::new(
                ResourceErrorKind::Conflict,
                "resource key already exists",
            )
            .with_field(&self.key, "must be unique"));
        }
        let mut normalized = Map::new();
        for header in &self.headers {
            normalized.insert(
                header.clone(),
                normalized_csv_value(object.get(header).unwrap_or(&Value::Null)),
            );
        }
        if normalized.get(&self.headers[key_index]).and_then(Value::as_str) != Some(key) {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "resource key must be a string",
            )
            .with_field(&self.key, "must be a string"));
        }
        validate_schema_value(&self.schema, &normalized)?;
        values.push(normalized.clone());
        self.write_values_locked(&values)?;
        Ok(Value::Object(normalized))
    }

    fn update(&self, id: &str, patch: Value) -> ResourceResult<Value> {
        self.ensure_writable()?;
        require_object_patch(&patch)?;
        let key_index = self.key_index()?;
        let _guard = self.write_lock.lock().map_err(|_| {
            ResourceError::new(ResourceErrorKind::Internal, "data write lock is poisoned")
        })?;
        let mut values = self.read_values()?;
        let position = values
            .iter()
            .position(|value| value.get(&self.key).and_then(Value::as_str) == Some(id))
            .ok_or_else(|| {
                ResourceError::new(ResourceErrorKind::NotFound, "resource item was not found")
            })?;
        validate_columns(&self.headers, patch.as_object().expect("object patch validated"))?;
        let merged = apply_merge_patch(Value::Object(values[position].clone()), &patch)?;
        let merged_object = object_value(merged, "updated value")?;
        let merged_key = required_key(&merged_object, &self.key)?;
        if merged_key != id {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "resource primary key cannot be changed",
            )
            .with_field(&self.key, "must match the item id"));
        }
        let mut normalized = Map::new();
        for header in &self.headers {
            normalized.insert(
                header.clone(),
                normalized_csv_value(merged_object.get(header).unwrap_or(&Value::Null)),
            );
        }
        if normalized.get(&self.headers[key_index]).and_then(Value::as_str) != Some(id) {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "resource key must be a string",
            ));
        }
        validate_schema_value(&self.schema, &normalized)?;
        values[position] = normalized.clone();
        self.write_values_locked(&values)?;
        Ok(Value::Object(normalized))
    }

    fn delete(&self, id: &str) -> ResourceResult<()> {
        self.ensure_writable()?;
        self.key_index()?;
        let _guard = self.write_lock.lock().map_err(|_| {
            ResourceError::new(ResourceErrorKind::Internal, "data write lock is poisoned")
        })?;
        let mut values = self.read_values()?;
        let position = values
            .iter()
            .position(|value| value.get(&self.key).and_then(Value::as_str) == Some(id))
            .ok_or_else(|| {
                ResourceError::new(ResourceErrorKind::NotFound, "resource item was not found")
            })?;
        values.remove(position);
        self.write_values_locked(&values)
    }
}

impl ResourceProvider for CsvBackend {
    type Item = Value;

    fn schema(&self) -> ResourceResult<ResourceSchema> {
        JsonResourceProvider::schema(self)
    }

    fn list(&self, query: &ListQuery) -> ResourceResult<ResourcePage<Self::Item>> {
        JsonResourceProvider::list(self, query)
    }

    fn get(&self, id: &str) -> ResourceResult<Option<Self::Item>> {
        JsonResourceProvider::get(self, id)
    }

    fn create(&mut self, value: Self::Item) -> ResourceResult<Self::Item> {
        JsonResourceProvider::create(self, value)
    }

    fn update(&mut self, id: &str, patch: Self::Item) -> ResourceResult<Self::Item> {
        JsonResourceProvider::update(self, id, patch)
    }

    fn delete(&mut self, id: &str) -> ResourceResult<()> {
        JsonResourceProvider::delete(self, id)
    }
}

#[derive(Debug)]
enum DataBackend {
    Csv(CsvBackend),
    Parquet(ParquetBackend),
}

/// A local file-backed provider that dispatches between CSV and Parquet.
pub struct DataResourceProvider {
    format: DataFormat,
    backend: DataBackend,
}

impl std::fmt::Debug for DataResourceProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DataResourceProvider")
            .field("format", &self.format)
            .field("backend", &self.backend)
            .finish()
    }
}

impl DataResourceProvider {
    /// Opens and validates a provider selected by explicit format or extension.
    pub fn new(config: DataResourceConfig) -> ResourceResult<Self> {
        let format = config.resolved_format().map_err(config_error)?;
        let backend = match format {
            DataFormat::Csv => DataBackend::Csv(CsvBackend::new(config)?),
            DataFormat::Parquet => DataBackend::Parquet(ParquetBackend::new(config)?),
        };
        Ok(Self { format, backend })
    }

    /// Alias for [`Self::new`] that reads naturally at call sites.
    pub fn open(config: DataResourceConfig) -> ResourceResult<Self> {
        Self::new(config)
    }

    /// Returns the selected data format.
    #[must_use]
    pub const fn format(&self) -> DataFormat {
        self.format
    }

    /// Returns the canonical file path used by this provider.
    #[must_use]
    pub fn path(&self) -> &Path {
        match &self.backend {
            DataBackend::Csv(provider) => provider.path(),
            DataBackend::Parquet(provider) => provider.path(),
        }
    }
}

impl JsonResourceProvider for DataResourceProvider {
    fn schema(&self) -> ResourceResult<ResourceSchema> {
        match &self.backend {
            DataBackend::Csv(provider) => JsonResourceProvider::schema(provider),
            DataBackend::Parquet(provider) => JsonResourceProvider::schema(provider),
        }
    }

    fn list(&self, query: &ListQuery) -> ResourceResult<ResourcePage<Value>> {
        match &self.backend {
            DataBackend::Csv(provider) => JsonResourceProvider::list(provider, query),
            DataBackend::Parquet(provider) => JsonResourceProvider::list(provider, query),
        }
    }

    fn get(&self, id: &str) -> ResourceResult<Option<Value>> {
        match &self.backend {
            DataBackend::Csv(provider) => JsonResourceProvider::get(provider, id),
            DataBackend::Parquet(provider) => JsonResourceProvider::get(provider, id),
        }
    }

    fn create(&self, value: Value) -> ResourceResult<Value> {
        match &self.backend {
            DataBackend::Csv(provider) => JsonResourceProvider::create(provider, value),
            DataBackend::Parquet(provider) => JsonResourceProvider::create(provider, value),
        }
    }

    fn update(&self, id: &str, patch: Value) -> ResourceResult<Value> {
        match &self.backend {
            DataBackend::Csv(provider) => JsonResourceProvider::update(provider, id, patch),
            DataBackend::Parquet(provider) => JsonResourceProvider::update(provider, id, patch),
        }
    }

    fn delete(&self, id: &str) -> ResourceResult<()> {
        match &self.backend {
            DataBackend::Csv(provider) => JsonResourceProvider::delete(provider, id),
            DataBackend::Parquet(provider) => JsonResourceProvider::delete(provider, id),
        }
    }
}

impl ResourceProvider for DataResourceProvider {
    type Item = Value;

    fn schema(&self) -> ResourceResult<ResourceSchema> {
        JsonResourceProvider::schema(self)
    }

    fn list(&self, query: &ListQuery) -> ResourceResult<ResourcePage<Self::Item>> {
        JsonResourceProvider::list(self, query)
    }

    fn get(&self, id: &str) -> ResourceResult<Option<Self::Item>> {
        JsonResourceProvider::get(self, id)
    }

    fn create(&mut self, value: Self::Item) -> ResourceResult<Self::Item> {
        JsonResourceProvider::create(self, value)
    }

    fn update(&mut self, id: &str, patch: Self::Item) -> ResourceResult<Self::Item> {
        JsonResourceProvider::update(self, id, patch)
    }

    fn delete(&mut self, id: &str) -> ResourceResult<()> {
        JsonResourceProvider::delete(self, id)
    }
}

fn config_error(error: DataConfigError) -> ResourceError {
    ResourceError::new(ResourceErrorKind::Validation, error.message())
}

fn io_error(operation: &str, error: io::Error) -> ResourceError {
    ResourceError::new(ResourceErrorKind::Unavailable, format!("could not {operation}"))
        .with_field("storage", error.kind().to_string())
}

fn path_lock(path: &Path) -> Arc<Mutex<()>> {
    let locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks.lock().expect("global data lock registry is not poisoned");
    locks.entry(path.to_owned()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
}

fn read_rows(path: &Path) -> ResourceResult<(Vec<String>, Vec<StringRecord>)> {
    let file = File::open(path).map_err(|error| io_error("read CSV", error))?;
    let mut reader = ReaderBuilder::new().has_headers(true).flexible(false).from_reader(file);
    let headers = reader
        .headers()
        .map_err(|error| csv_error("read data headers", error))?
        .iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if headers.is_empty() || headers.iter().any(|header| header.trim().is_empty()) {
        return Err(ResourceError::new(
            ResourceErrorKind::Validation,
            "data headers must not be empty",
        ));
    }
    let mut unique = std::collections::BTreeSet::new();
    if headers.iter().any(|header| !unique.insert(header)) {
        return Err(ResourceError::new(
            ResourceErrorKind::Validation,
            "data headers must be unique",
        ));
    }
    let mut records = Vec::new();
    for record in reader.records() {
        records.push(record.map_err(|error| csv_error("read CSV rows", error))?);
    }
    Ok((headers, records))
}

fn csv_error(operation: &str, error: csv::Error) -> ResourceError {
    let _ = error;
    ResourceError::new(ResourceErrorKind::Validation, operation)
        .with_field("csv", "invalid CSV structure")
}

fn validate_unique_keys(
    records: &[StringRecord],
    key_index: usize,
    key: &str,
) -> ResourceResult<()> {
    let mut keys = std::collections::BTreeSet::new();
    for record in records {
        let value = record.get(key_index).unwrap_or_default();
        if value.is_empty() {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "data primary keys must not be empty",
            )
            .with_field(key, "must not be empty"));
        }
        if !keys.insert(value) {
            return Err(ResourceError::new(
                ResourceErrorKind::Conflict,
                "data contains duplicate primary keys",
            )
            .with_field(key, "must be unique"));
        }
    }
    Ok(())
}

fn validate_unique_value_keys(values: &[Map<String, Value>], key: &str) -> ResourceResult<()> {
    let mut keys = std::collections::BTreeSet::new();
    for value in values {
        let key_value = value.get(key).and_then(Value::as_str).filter(|value| !value.is_empty());
        if key_value.is_none() || !keys.insert(key_value) {
            return Err(ResourceError::new(
                ResourceErrorKind::Conflict,
                "data values contain duplicate or empty primary keys",
            )
            .with_field(key, "must be unique and non-empty"));
        }
    }
    Ok(())
}

fn record_to_object(headers: &[String], record: &StringRecord) -> Map<String, Value> {
    headers
        .iter()
        .zip(record.iter())
        .map(|(header, value)| (header.clone(), Value::String(value.to_owned())))
        .collect()
}

fn object_value(value: Value, label: &str) -> ResourceResult<Map<String, Value>> {
    value.as_object().cloned().ok_or_else(|| {
        ResourceError::new(ResourceErrorKind::Validation, format!("{label} must be a JSON object"))
    })
}

fn required_key<'a>(object: &'a Map<String, Value>, key: &str) -> ResourceResult<&'a str> {
    object.get(key).and_then(Value::as_str).filter(|value| !value.is_empty()).ok_or_else(|| {
        ResourceError::new(ResourceErrorKind::Validation, "resource primary key is required")
            .with_field(key, "must be a non-empty string")
    })
}

fn validate_columns(headers: &[String], object: &Map<String, Value>) -> ResourceResult<()> {
    for field in object.keys() {
        if !headers.iter().any(|header| header == field) {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "value contains an unknown data column",
            )
            .with_field(field, "column does not exist"));
        }
        if object[field].is_object() || object[field].is_array() {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "data values must be scalar",
            )
            .with_field(field, "must be a string, number, boolean, or null"));
        }
    }
    Ok(())
}

fn validate_schema_value(
    schema: &ResourceSchema,
    object: &Map<String, Value>,
) -> ResourceResult<()> {
    for field in &schema.fields {
        let value = object.get(&field.name).unwrap_or(&Value::Null);
        if value.is_null() || value.as_str() == Some("") {
            if field.required {
                return Err(ResourceError::new(
                    ResourceErrorKind::Validation,
                    "resource field is required",
                )
                .with_field(&field.name, "must not be empty"));
            }
            continue;
        }
        let valid_type = match field.field_type {
            FieldType::Text | FieldType::Date | FieldType::Json => value.is_string(),
            FieldType::Number => value.as_str().is_some_and(|value| value.parse::<f64>().is_ok()),
            FieldType::Integer => value.as_str().is_some_and(|value| value.parse::<i64>().is_ok()),
            FieldType::Boolean => {
                value.as_str().is_some_and(|value| matches!(value, "true" | "false"))
            }
        };
        if !valid_type {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "resource field does not match its schema type",
            )
            .with_field(&field.name, "has an invalid type"));
        }
        if let Some(enum_values) = &field.enum_values
            && !enum_values.iter().any(|expected| csv_enum_matches(value, expected))
        {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "resource field is not in its schema enum",
            )
            .with_field(&field.name, "must be one of the declared values"));
        }
        if let Some(format) = &field.format
            && !csv_format_matches(value, format)
        {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "resource field does not match its schema format",
            )
            .with_field(&field.name, "has an invalid format"));
        }
    }
    Ok(())
}

fn csv_enum_matches(value: &Value, expected: &Value) -> bool {
    match expected {
        Value::String(expected) => value.as_str() == Some(expected),
        Value::Number(expected) => {
            value.as_str().is_some_and(|value| value.parse::<f64>().ok() == expected.as_f64())
        }
        Value::Bool(expected) => {
            value.as_str().and_then(|value| value.parse::<bool>().ok()) == Some(*expected)
        }
        Value::Null => value.is_null(),
        _ => value == expected,
    }
}

fn csv_format_matches(value: &Value, format: &str) -> bool {
    let Some(value) = value.as_str() else { return false };
    match format {
        "email" => {
            let Some((local, domain)) = value.split_once('@') else { return false };
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !value.chars().any(char::is_whitespace)
        }
        "date" => valid_date(value),
        "date-time" => {
            let Some((date, time)) = value.split_once('T') else { return false };
            valid_date(date) && valid_time(time)
        }
        _ => false,
    }
}

fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        return false;
    }
    let year = value[0..4].parse::<u32>().ok();
    let month = value[5..7].parse::<u32>().ok();
    let day = value[8..10].parse::<u32>().ok();
    let (Some(year), Some(month), Some(day)) = (year, month, day) else { return false };
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

fn valid_time(value: &str) -> bool {
    let (clock, timezone) = if let Some(clock) = value.strip_suffix('Z') {
        (clock, None)
    } else if let Some(position) = value.rfind(['+', '-']) {
        (&value[..position], Some(&value[position..]))
    } else {
        (value, None)
    };
    let mut parts = clock.split(':');
    let (Some(hour), Some(minute), Some(seconds), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let (seconds, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
    let valid_digits =
        |value: &str| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
    if hour.len() != 2
        || minute.len() != 2
        || seconds.len() != 2
        || !valid_digits(hour)
        || !valid_digits(minute)
        || !valid_digits(seconds)
        || (!fraction.is_empty() && !valid_digits(fraction))
    {
        return false;
    }
    let valid_clock = hour.parse::<u32>().is_ok_and(|value| value <= 23)
        && minute.parse::<u32>().is_ok_and(|value| value <= 59)
        && seconds.parse::<u32>().is_ok_and(|value| value <= 59);
    let valid_timezone = timezone.is_none_or(|zone| {
        if zone.len() != 6 || zone.as_bytes()[3] != b':' {
            return false;
        }
        (zone.starts_with('+') || zone.starts_with('-'))
            && zone[1..3].parse::<u32>().is_ok_and(|value| value <= 23)
            && zone[4..6].parse::<u32>().is_ok_and(|value| value <= 59)
    });
    valid_clock && valid_timezone
}

fn compare_values(left: &Value, right: &Value, sorts: &[egake_resource::Sort]) -> Ordering {
    sorts
        .iter()
        .find_map(|sort| {
            let left = left.get(&sort.field).and_then(Value::as_str).unwrap_or_default();
            let right = right.get(&sort.field).and_then(Value::as_str).unwrap_or_default();
            let ordering = left.cmp(right);
            (ordering != Ordering::Equal).then_some(match sort.direction {
                SortDirection::Ascending => ordering,
                SortDirection::Descending => ordering.reverse(),
            })
        })
        .unwrap_or(Ordering::Equal)
}

fn write_csv(path: &Path, headers: &[String], values: &[Map<String, Value>]) -> io::Result<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut writer = WriterBuilder::new().has_headers(true).from_writer(file);
    writer.write_record(headers).map_err(csv_to_io)?;
    for value in values {
        let row = headers.iter().map(|header| csv_value(value.get(header).unwrap_or(&Value::Null)));
        writer.write_record(row).map_err(csv_to_io)?;
    }
    let mut file = writer.into_inner().map_err(|error| error.into_error())?;
    file.flush()?;
    file.sync_all()
}

fn csv_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => String::new(),
    }
}

fn normalized_csv_value(value: &Value) -> Value {
    Value::String(csv_value(value))
}

fn csv_to_io(error: csv::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn temporary_path(path: &Path) -> PathBuf {
    let counter = TEMP_COUNTER
        .get_or_init(|| std::sync::atomic::AtomicU64::new(0))
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("resource.csv");
    path.with_file_name(format!(".{file_name}.egake-{stamp}-{counter}.tmp"))
}

fn retain_backups(path: &Path, backup_count: u8) -> io::Result<()> {
    if backup_count == 0 {
        return Ok(());
    }
    for index in (1..backup_count).rev() {
        let old = backup_path(path, index);
        let new = backup_path(path, index + 1);
        if old.exists() {
            remove_backup_destination(&new)?;
            fs::rename(old, new)?;
        }
    }
    remove_backup_destination(&backup_path(path, 1))?;
    fs::copy(path, backup_path(path, 1))?;
    Ok(())
}

fn remove_backup_destination(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path)
    } else {
        Err(io::Error::new(io::ErrorKind::AlreadyExists, "data backup path is not a regular file"))
    }
}

fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    File::open(parent)?.sync_all()
}

fn backup_path(path: &Path, index: u8) -> PathBuf {
    PathBuf::from(format!("{}.bak.{index}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc, thread};

    use egake_resource::{Capability, JsonResourceProvider, ListQuery, ResourceErrorKind, Sort};
    use tempfile::tempdir;

    use super::*;

    fn provider(contents: &str, writable: bool) -> (tempfile::TempDir, CsvBackend) {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("contacts.csv");
        fs::write(&path, contents).expect("CSV fixture");
        let provider = CsvBackend::new(
            DataResourceConfig::new(path).with_writable(writable).with_backup_count(2),
        )
        .expect("provider");
        (directory, provider)
    }

    #[test]
    fn contract_supports_search_sort_and_pagination() {
        let (_directory, provider) = provider(
            "id,name,email\n1,Ada,ada@example.com\n2,Grace,grace@example.com\n3,Alan,alan@example.com\n",
            false,
        );
        let schema = JsonResourceProvider::schema(&provider).expect("schema");
        assert!(schema.capabilities.contains(&Capability::List));
        let page = JsonResourceProvider::list(
            &provider,
            &ListQuery::new()
                .with_search("a")
                .with_pagination(1, 1)
                .then_sort(Sort::ascending("name")),
        )
        .expect("page");
        assert_eq!(page.total, 3);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0]["name"], "Alan");
    }

    #[test]
    fn contract_supports_create_update_delete_and_backups() {
        let (directory, provider) = provider("id,name\n1,Ada\n", true);
        let created =
            provider.create(serde_json::json!({"id":"2", "name":"Grace"})).expect("create");
        assert_eq!(created["name"], "Grace");
        let updated =
            provider.update("2", serde_json::json!({"name":"Grace Hopper"})).expect("update");
        assert_eq!(updated["name"], "Grace Hopper");
        provider.delete("1").expect("delete");
        assert!(JsonResourceProvider::get(&provider, "1").expect("get").is_none());
        assert!(directory.path().join("contacts.csv.bak.1").is_file());
    }

    #[test]
    fn malformed_and_unsupported_operations_are_structured() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("duplicates.csv");
        fs::write(&path, "id,name\n1,Ada\n1,Duplicate\n").expect("CSV fixture");
        let error = CsvBackend::new(DataResourceConfig::new(path)).expect_err("duplicate keys");
        assert_eq!(error.kind, ResourceErrorKind::Conflict);

        let (_directory, read_only) = provider("id,name\n1,Ada\n", false);
        let error = read_only.create(serde_json::json!({"id":"2"})).expect_err("read-only write");
        assert_eq!(error.kind, ResourceErrorKind::CapabilityDenied);
    }

    #[test]
    fn writable_provider_requires_key_and_rejects_traversal() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("records.csv");
        fs::write(&path, "name\nAda\n").expect("CSV fixture");
        let error = CsvBackend::new(DataResourceConfig::new(path).with_writable(true))
            .expect_err("missing key");
        assert_eq!(error.kind, ResourceErrorKind::Validation);

        let config = DataResourceConfig::new("../records.csv");
        assert!(config.validate().is_err());
    }

    #[test]
    fn external_schema_metadata_reaches_json_boundary_and_validates_writes() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("contacts.csv");
        fs::write(&path, "id,email,status\n1,ada@example.com,active\n").expect("CSV fixture");
        let mut external = ResourceSchema::new("contacts");
        external.push_field(FieldSchema::new("id", FieldType::Text).required());
        external
            .push_field(FieldSchema::new("email", FieldType::Text).with_format("email").required());
        external.push_field(
            FieldSchema::new("status", FieldType::Text)
                .with_enum_values(vec![serde_json::json!("active"), serde_json::json!("paused")]),
        );
        let provider = CsvBackend::new(
            DataResourceConfig::new(path).with_writable(true).with_schema(external),
        )
        .expect("provider");

        let schema = JsonResourceProvider::schema(&provider).expect("schema");
        let email = schema.fields.iter().find(|field| field.name == "email").expect("email");
        assert_eq!(email.format.as_deref(), Some("email"));
        assert_eq!(schema.fields[2].enum_values.as_ref().expect("enum").len(), 2);
        let wire = serde_json::to_value(&schema).expect("schema JSON");
        assert_eq!(wire["fields"][1]["format"], "email");
        assert_eq!(wire["fields"][2]["enum"][0], "active");

        let error = provider
            .create(serde_json::json!({"id":"2", "email":"not-an-email", "status":"active"}))
            .expect_err("invalid email");
        assert_eq!(error.kind, ResourceErrorKind::Validation);
        assert!(error.fields.contains_key("email"));
        let error = provider
            .create(serde_json::json!({"id":"2", "email":"grace@example.com", "status":"unknown"}))
            .expect_err("invalid enum");
        assert_eq!(error.kind, ResourceErrorKind::Validation);
        assert!(error.fields.contains_key("status"));
    }

    #[test]
    fn concurrent_providers_share_the_path_lock() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("concurrent.csv");
        fs::write(&path, "id,name\n0,Start\n").expect("CSV fixture");
        let first = Arc::new(
            CsvBackend::new(DataResourceConfig::new(&path).with_writable(true)).expect("provider"),
        );
        let second = Arc::new(
            CsvBackend::new(DataResourceConfig::new(&path).with_writable(true)).expect("provider"),
        );
        let handles = [first, second].into_iter().enumerate().map(|(offset, provider)| {
            thread::spawn(move || {
                for index in 0..8 {
                    let id = (offset * 8 + index + 1).to_string();
                    provider
                        .create(serde_json::json!({"id": id, "name": "worker"}))
                        .expect("create");
                }
            })
        });
        for handle in handles {
            handle.join().expect("worker");
        }
        let reader = CsvBackend::new(DataResourceConfig::new(&path)).expect("reader");
        let page = JsonResourceProvider::list(&reader, &ListQuery::new().with_pagination(0, 500))
            .expect("list");
        assert_eq!(page.total, 17);
    }

    #[test]
    fn backup_rotation_preserves_recoverable_generations() {
        let (directory, provider) = provider("id,name\n1,Ada\n", true);
        provider.update("1", serde_json::json!({"name": "Grace"})).expect("update");
        provider.update("1", serde_json::json!({"name": "Katherine"})).expect("update");
        assert_eq!(
            fs::read_to_string(directory.path().join("contacts.csv.bak.1")).expect("backup"),
            "id,name\n1,Grace\n"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("contacts.csv.bak.2")).expect("backup"),
            "id,name\n1,Ada\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn backup_symlinks_cannot_redirect_a_write() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("contacts.csv");
        let outside = directory.path().join("outside.txt");
        fs::write(&path, "id,name\n1,Ada\n").expect("CSV fixture");
        fs::write(&outside, "do not overwrite").expect("outside fixture");
        symlink(&outside, directory.path().join("contacts.csv.bak.1")).expect("symlink");
        let provider = CsvBackend::new(
            DataResourceConfig::new(&path).with_writable(true).with_backup_count(1),
        )
        .expect("provider");
        provider.update("1", serde_json::json!({"name": "Grace"})).expect("update");
        assert_eq!(fs::read_to_string(outside).expect("outside"), "do not overwrite");
        assert_eq!(
            fs::read_to_string(directory.path().join("contacts.csv.bak.1")).expect("backup"),
            "id,name\n1,Ada\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn replacement_preserves_data_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("contacts.csv");
        fs::write(&path, "id,name\n1,Ada\n").expect("CSV fixture");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("permissions");
        let provider =
            CsvBackend::new(DataResourceConfig::new(&path).with_writable(true)).expect("provider");
        provider.update("1", serde_json::json!({"name": "Grace"})).expect("update");
        assert_eq!(fs::metadata(path).expect("metadata").permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn generic_provider_dispatch_preserves_csv_contract() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("records.csv");
        fs::write(&path, "id,name\n1,Ada\n").expect("fixture");
        let provider =
            DataResourceProvider::open(DataResourceConfig::new(&path)).expect("generic provider");
        assert_eq!(provider.format(), DataFormat::Csv);
        assert!(
            JsonResourceProvider::schema(&provider)
                .expect("schema")
                .capabilities
                .contains(&Capability::Get)
        );
        assert_eq!(
            JsonResourceProvider::get(&provider, "1").expect("get").expect("record")["name"],
            "Ada"
        );
    }
}
