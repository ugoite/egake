//! A small, local-file CSV implementation of the JSON Resource Contract.

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
use ikashita_resource::{
    Capability, FieldSchema, FieldType, JsonResourceProvider, ListQuery, ResourceError,
    ResourceErrorKind, ResourcePage, ResourceProvider, ResourceResult, ResourceSchema,
    SortDirection, apply_merge_patch, require_object_patch,
};
use serde_json::{Map, Value};

pub use config::{CsvConfigError, CsvResourceConfig, DEFAULT_RESOURCE_KEY};

static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
static TEMP_COUNTER: OnceLock<std::sync::atomic::AtomicU64> = OnceLock::new();

/// A file-backed provider for small local CSV datasets.
pub struct CsvResourceProvider {
    path: PathBuf,
    key: String,
    headers: Vec<String>,
    schema: ResourceSchema,
    write_lock: Arc<Mutex<()>>,
    writable: bool,
    backup_count: u8,
}

impl std::fmt::Debug for CsvResourceProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CsvResourceProvider")
            .field("path", &self.path)
            .field("key", &self.key)
            .field("headers", &self.headers)
            .field("writable", &self.writable)
            .field("backup_count", &self.backup_count)
            .finish_non_exhaustive()
    }
}

impl CsvResourceProvider {
    /// Opens and validates a CSV provider.
    pub fn new(config: CsvResourceConfig) -> ResourceResult<Self> {
        config.validate().map_err(config_error)?;
        let path = fs::canonicalize(config.path()).map_err(|error| io_error("open CSV", error))?;
        let metadata = fs::metadata(&path).map_err(|error| io_error("inspect CSV", error))?;
        if !metadata.is_file() {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "CSV path must refer to a regular file",
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

        let name = config
            .name()
            .map(str::to_owned)
            .or_else(|| path.file_stem().and_then(|stem| stem.to_str()).map(str::to_owned))
            .unwrap_or_else(|| "resource".to_owned());
        let mut schema = ResourceSchema::new(name);
        for header in &headers {
            let field = FieldSchema::new(header, FieldType::Text);
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

    /// Alias for Self::new that reads naturally at call sites.
    pub fn open(config: CsvResourceConfig) -> ResourceResult<Self> {
        Self::new(config)
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
                "CSV resource has no configured primary-key column",
            )
        })
    }

    fn ensure_writable(&self) -> ResourceResult<()> {
        if self.writable {
            Ok(())
        } else {
            Err(ResourceError::new(
                ResourceErrorKind::CapabilityDenied,
                "CSV resource is read-only",
            ))
        }
    }

    fn read_values(&self) -> ResourceResult<Vec<Map<String, Value>>> {
        let (headers, records) = read_rows(&self.path)?;
        if headers != self.headers {
            return Err(ResourceError::new(
                ResourceErrorKind::Conflict,
                "CSV headers changed while the provider was open",
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
                "CSV headers changed while the provider was open",
            ));
        }
        if let Some(key_index) = self.headers.iter().position(|header| header == &self.key) {
            validate_unique_keys(&current_records, key_index, &self.key)?;
        }
        let temp_path = temporary_path(&self.path);
        let write_result = write_csv(&temp_path, &self.headers, values)
            .and_then(|()| retain_backups(&self.path, self.backup_count))
            .and_then(|()| fs::rename(&temp_path, &self.path));
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result.map_err(|error| io_error("write CSV", error))
    }
}

impl JsonResourceProvider for CsvResourceProvider {
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
            ResourceError::new(ResourceErrorKind::Internal, "CSV write lock is poisoned")
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
            .with_field(&self.key, key));
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
        values.push(normalized.clone());
        self.write_values_locked(&values)?;
        Ok(Value::Object(normalized))
    }

    fn update(&self, id: &str, patch: Value) -> ResourceResult<Value> {
        self.ensure_writable()?;
        require_object_patch(&patch)?;
        let key_index = self.key_index()?;
        let _guard = self.write_lock.lock().map_err(|_| {
            ResourceError::new(ResourceErrorKind::Internal, "CSV write lock is poisoned")
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
        values[position] = normalized.clone();
        self.write_values_locked(&values)?;
        Ok(Value::Object(normalized))
    }

    fn delete(&self, id: &str) -> ResourceResult<()> {
        self.ensure_writable()?;
        self.key_index()?;
        let _guard = self.write_lock.lock().map_err(|_| {
            ResourceError::new(ResourceErrorKind::Internal, "CSV write lock is poisoned")
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

impl ResourceProvider for CsvResourceProvider {
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

fn config_error(error: CsvConfigError) -> ResourceError {
    ResourceError::new(ResourceErrorKind::Validation, error.message())
}

fn io_error(operation: &str, error: io::Error) -> ResourceError {
    ResourceError::new(ResourceErrorKind::Unavailable, format!("could not {operation}"))
        .with_field("storage", error.kind().to_string())
}

fn path_lock(path: &Path) -> Arc<Mutex<()>> {
    let locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = locks.lock().expect("global CSV lock registry is not poisoned");
    locks.entry(path.to_owned()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
}

fn read_rows(path: &Path) -> ResourceResult<(Vec<String>, Vec<StringRecord>)> {
    let file = File::open(path).map_err(|error| io_error("read CSV", error))?;
    let mut reader = ReaderBuilder::new().has_headers(true).flexible(false).from_reader(file);
    let headers = reader
        .headers()
        .map_err(|error| csv_error("read CSV headers", error))?
        .iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if headers.is_empty() || headers.iter().any(|header| header.trim().is_empty()) {
        return Err(ResourceError::new(
            ResourceErrorKind::Validation,
            "CSV headers must not be empty",
        ));
    }
    let mut unique = std::collections::BTreeSet::new();
    if headers.iter().any(|header| !unique.insert(header)) {
        return Err(ResourceError::new(
            ResourceErrorKind::Validation,
            "CSV headers must be unique",
        ));
    }
    let mut records = Vec::new();
    for record in reader.records() {
        records.push(record.map_err(|error| csv_error("read CSV rows", error))?);
    }
    Ok((headers, records))
}

fn csv_error(operation: &str, error: csv::Error) -> ResourceError {
    ResourceError::new(ResourceErrorKind::Validation, operation)
        .with_field("csv", error.to_string())
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
                "CSV primary keys must not be empty",
            )
            .with_field(key, "must not be empty"));
        }
        if !keys.insert(value) {
            return Err(ResourceError::new(
                ResourceErrorKind::Conflict,
                "CSV contains duplicate primary keys",
            )
            .with_field(key, value));
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
                "value contains an unknown CSV column",
            )
            .with_field(field, "column does not exist"));
        }
        if object[field].is_object() || object[field].is_array() {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "CSV values must be scalar",
            )
            .with_field(field, "must be a string, number, boolean, or null"));
        }
    }
    Ok(())
}

fn compare_values(left: &Value, right: &Value, sorts: &[ikashita_resource::Sort]) -> Ordering {
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
    path.with_file_name(format!(".{file_name}.ikashita-{stamp}-{counter}.tmp"))
}

fn retain_backups(path: &Path, backup_count: u8) -> io::Result<()> {
    if backup_count == 0 {
        return Ok(());
    }
    for index in (1..backup_count).rev() {
        let old = backup_path(path, index);
        let new = backup_path(path, index + 1);
        if old.exists() {
            fs::rename(old, new)?;
        }
    }
    fs::copy(path, backup_path(path, 1))?;
    Ok(())
}

fn backup_path(path: &Path, index: u8) -> PathBuf {
    PathBuf::from(format!("{}.bak.{index}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ikashita_resource::{Capability, JsonResourceProvider, ListQuery, ResourceErrorKind, Sort};
    use tempfile::tempdir;

    use super::*;

    fn provider(contents: &str, writable: bool) -> (tempfile::TempDir, CsvResourceProvider) {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("contacts.csv");
        fs::write(&path, contents).expect("CSV fixture");
        let provider = CsvResourceProvider::new(
            CsvResourceConfig::new(path).with_writable(writable).with_backup_count(2),
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
        let error =
            CsvResourceProvider::new(CsvResourceConfig::new(path)).expect_err("duplicate keys");
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
        let error = CsvResourceProvider::new(CsvResourceConfig::new(path).with_writable(true))
            .expect_err("missing key");
        assert_eq!(error.kind, ResourceErrorKind::Validation);

        let config = CsvResourceConfig::new("../records.csv");
        assert!(config.validate().is_err());
    }
}
