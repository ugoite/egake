//! Read-only Parquet backend and Arrow-to-JSON conversion.

use std::{
    cmp::Ordering,
    collections::BTreeSet,
    fs::{self, File},
    path::{Path, PathBuf},
};

use arrow_array::{
    Array, ArrayRef, RecordBatch,
    cast::AsArray,
    types::{
        Date32Type, Date64Type, Decimal32Type, Decimal64Type, Decimal128Type, Decimal256Type,
        Float16Type, Float32Type, Float64Type, Int8Type, Int16Type, Int32Type, Int64Type,
        Time32MillisecondType, Time32SecondType, Time64MicrosecondType, Time64NanosecondType,
        TimestampMicrosecondType, TimestampMillisecondType, TimestampNanosecondType,
        TimestampSecondType, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
    },
};
use arrow_schema::{DataType, TimeUnit};
use ikashita_resource::{
    Capability, FieldSchema, FieldType, JsonResourceProvider, ListQuery, ResourceError,
    ResourceErrorKind, ResourcePage, ResourceProvider, ResourceResult, ResourceSchema,
    SortDirection,
};
use serde_json::{Map, Number, Value};

use crate::{DataFormat, DataResourceConfig};

/// The in-memory, read-only representation of one Parquet resource.
#[derive(Debug)]
pub(crate) struct ParquetBackend {
    path: PathBuf,
    key: String,
    fields: Vec<String>,
    schema: ResourceSchema,
    values: Vec<Map<String, Value>>,
}

impl ParquetBackend {
    pub(crate) fn new(config: DataResourceConfig) -> ResourceResult<Self> {
        config.validate().map_err(config_error)?;
        if config.resolved_format().map_err(config_error)? != DataFormat::Parquet {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "Parquet backend requires Parquet format",
            ));
        }
        let path =
            fs::canonicalize(config.path()).map_err(|error| io_error("open Parquet", error))?;
        let metadata = fs::metadata(&path).map_err(|error| io_error("inspect Parquet", error))?;
        if !metadata.is_file() {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "Parquet path must refer to a regular file",
            ));
        }

        let file = File::open(&path).map_err(|error| io_error("read Parquet", error))?;
        let builder = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)
            .map_err(|error| parquet_error("read Parquet metadata", error))?;
        let arrow_schema = builder.schema().clone();
        let configured_schema = config.schema().cloned();
        let mut fields = Vec::with_capacity(arrow_schema.fields().len());
        let mut schema = ResourceSchema::new(
            config
                .name()
                .map(str::to_owned)
                .or_else(|| path.file_stem().and_then(|stem| stem.to_str()).map(str::to_owned))
                .unwrap_or_else(|| "resource".to_owned()),
        );
        let mut names = BTreeSet::new();
        for field in arrow_schema.fields() {
            if field.name().trim().is_empty() || !names.insert(field.name()) {
                return Err(ResourceError::new(
                    ResourceErrorKind::Validation,
                    "Parquet fields must have unique non-empty names",
                ));
            }
            let inferred_type = field_type_for_arrow(field.data_type())?;
            let declaration = configured_schema
                .as_ref()
                .and_then(|configured| {
                    configured.fields.iter().find(|item| item.name == *field.name())
                })
                .cloned()
                .unwrap_or_else(|| FieldSchema::new(field.name(), inferred_type));
            fields.push(field.name().to_owned());
            schema.push_field(if field.name() == config.key() {
                declaration.required()
            } else {
                declaration
            });
        }
        if let Some(configured) = &configured_schema {
            for field in &configured.fields {
                if !fields.iter().any(|name| name == &field.name) {
                    return Err(ResourceError::new(
                        ResourceErrorKind::Validation,
                        "Parquet is missing a schema column",
                    )
                    .with_field(&field.name, "column does not exist"));
                }
            }
        }

        let key_index = fields.iter().position(|field| field == config.key());
        if let Some(key_index) = key_index
            && !is_string_type(arrow_schema.field(key_index).data_type())
        {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "Parquet resource key column must use a string type",
            )
            .with_field("key", config.key()));
        }

        let reader = builder
            .with_batch_size(1024)
            .build()
            .map_err(|error| parquet_error("read Parquet rows", error))?;
        let mut values = Vec::new();
        for batch in reader {
            let batch = batch.map_err(|error| parquet_error("read Parquet rows", error))?;
            values.extend(batch_to_objects(&batch, &fields)?);
        }
        if let Some(key_index) = key_index {
            validate_unique_keys(&values, &fields[key_index])?;
            schema.grant(Capability::Get);
        }
        schema.grant(Capability::Schema);
        schema.grant(Capability::List);

        Ok(Self { path, key: config.key().to_owned(), fields, schema, values })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    fn key_index(&self) -> ResourceResult<usize> {
        self.fields.iter().position(|field| field == &self.key).ok_or_else(|| {
            ResourceError::new(
                ResourceErrorKind::CapabilityDenied,
                "Parquet resource has no configured string primary-key column",
            )
        })
    }

    fn read_only_error(operation: &str) -> ResourceError {
        ResourceError::new(
            ResourceErrorKind::CapabilityDenied,
            format!("Parquet resources are read-only; {operation} is not supported"),
        )
    }
}

impl JsonResourceProvider for ParquetBackend {
    fn schema(&self) -> ResourceResult<ResourceSchema> {
        Ok(self.schema.clone())
    }

    fn list(&self, query: &ListQuery) -> ResourceResult<ResourcePage<Value>> {
        for sort in &query.sort {
            if !self.fields.iter().any(|field| field == &sort.field) {
                return Err(ResourceError::new(
                    ResourceErrorKind::Validation,
                    "unknown sort field",
                )
                .with_field("sort", &sort.field));
            }
        }
        let needle = query.search.as_ref().map(|value| value.to_lowercase());
        let mut values: Vec<Value> = self
            .values
            .iter()
            .filter(|value| {
                needle.as_ref().is_none_or(|needle| {
                    value.values().any(|field| value_contains_text(field, needle))
                })
            })
            .cloned()
            .map(Value::Object)
            .collect();
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
            .values
            .iter()
            .find(|value| value.get(&self.fields[key]).and_then(Value::as_str) == Some(id))
            .cloned()
            .map(Value::Object))
    }

    fn create(&self, _value: Value) -> ResourceResult<Value> {
        Err(Self::read_only_error("create"))
    }

    fn update(&self, _id: &str, _patch: Value) -> ResourceResult<Value> {
        Err(Self::read_only_error("update"))
    }

    fn delete(&self, _id: &str) -> ResourceResult<()> {
        Err(Self::read_only_error("delete"))
    }
}

impl ResourceProvider for ParquetBackend {
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

fn batch_to_objects(
    batch: &RecordBatch,
    fields: &[String],
) -> ResourceResult<Vec<Map<String, Value>>> {
    (0..batch.num_rows())
        .map(|row| {
            fields
                .iter()
                .enumerate()
                .map(|(column, field)| {
                    array_value_to_json(batch.column(column), row)
                        .map(|value| (field.clone(), value))
                })
                .collect::<ResourceResult<Map<String, Value>>>()
        })
        .collect()
}

fn field_type_for_arrow(data_type: &DataType) -> ResourceResult<FieldType> {
    let field_type = match data_type {
        DataType::Boolean => FieldType::Boolean,
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => FieldType::Integer,
        DataType::Float16 | DataType::Float32 | DataType::Float64 => FieldType::Number,
        DataType::Decimal32(_, _)
        | DataType::Decimal64(_, _)
        | DataType::Decimal128(_, _)
        | DataType::Decimal256(_, _) => FieldType::Number,
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => FieldType::Text,
        DataType::Binary
        | DataType::LargeBinary
        | DataType::FixedSizeBinary(_)
        | DataType::BinaryView => FieldType::Json,
        DataType::Date32
        | DataType::Date64
        | DataType::Time32(_)
        | DataType::Time64(_)
        | DataType::Timestamp(_, _) => FieldType::Date,
        DataType::List(_)
        | DataType::LargeList(_)
        | DataType::FixedSizeList(_, _)
        | DataType::ListView(_)
        | DataType::LargeListView(_)
        | DataType::Struct(_)
        | DataType::Map(_, _)
        | DataType::Dictionary(_, _)
        | DataType::Null => FieldType::Json,
        other => {
            return Err(unsupported_type(other));
        }
    };
    Ok(field_type)
}

fn array_value_to_json(array: &ArrayRef, row: usize) -> ResourceResult<Value> {
    if array.is_null(row) {
        return Ok(Value::Null);
    }
    let value = match array.data_type() {
        DataType::Null => Value::Null,
        DataType::Boolean => Value::Bool(array.as_boolean().value(row)),
        DataType::Int8 => serde_json::json!(array.as_primitive::<Int8Type>().value(row)),
        DataType::Int16 => serde_json::json!(array.as_primitive::<Int16Type>().value(row)),
        DataType::Int32 => serde_json::json!(array.as_primitive::<Int32Type>().value(row)),
        DataType::Int64 => serde_json::json!(array.as_primitive::<Int64Type>().value(row)),
        DataType::UInt8 => serde_json::json!(array.as_primitive::<UInt8Type>().value(row)),
        DataType::UInt16 => serde_json::json!(array.as_primitive::<UInt16Type>().value(row)),
        DataType::UInt32 => serde_json::json!(array.as_primitive::<UInt32Type>().value(row)),
        DataType::UInt64 => serde_json::json!(array.as_primitive::<UInt64Type>().value(row)),
        DataType::Float16 => {
            number_from_f64(array.as_primitive::<Float16Type>().value(row).to_f32() as f64)?
        }
        DataType::Float32 => {
            number_from_f64(f64::from(array.as_primitive::<Float32Type>().value(row)))?
        }
        DataType::Float64 => number_from_f64(array.as_primitive::<Float64Type>().value(row))?,
        DataType::Decimal32(_, _)
        | DataType::Decimal64(_, _)
        | DataType::Decimal128(_, _)
        | DataType::Decimal256(_, _) => decimal_value(array, row)?,
        DataType::Utf8 => Value::String(array.as_string::<i32>().value(row).to_owned()),
        DataType::LargeUtf8 => Value::String(array.as_string::<i64>().value(row).to_owned()),
        DataType::Utf8View => Value::String(array.as_string_view().value(row).to_owned()),
        DataType::Binary => bytes_value(array.as_binary::<i32>().value(row)),
        DataType::LargeBinary => bytes_value(array.as_binary::<i64>().value(row)),
        DataType::FixedSizeBinary(_) => bytes_value(array.as_fixed_size_binary().value(row)),
        DataType::BinaryView => bytes_value(array.as_binary_view().value(row)),
        DataType::Date32 => serde_json::json!(array.as_primitive::<Date32Type>().value(row)),
        DataType::Date64 => serde_json::json!(array.as_primitive::<Date64Type>().value(row)),
        DataType::Time32(TimeUnit::Second) => {
            serde_json::json!(array.as_primitive::<Time32SecondType>().value(row))
        }
        DataType::Time32(TimeUnit::Millisecond) => {
            serde_json::json!(array.as_primitive::<Time32MillisecondType>().value(row))
        }
        DataType::Time64(TimeUnit::Microsecond) => {
            serde_json::json!(array.as_primitive::<Time64MicrosecondType>().value(row))
        }
        DataType::Time64(TimeUnit::Nanosecond) => {
            serde_json::json!(array.as_primitive::<Time64NanosecondType>().value(row))
        }
        DataType::Timestamp(TimeUnit::Second, _) => {
            serde_json::json!(array.as_primitive::<TimestampSecondType>().value(row))
        }
        DataType::Timestamp(TimeUnit::Millisecond, _) => {
            serde_json::json!(array.as_primitive::<TimestampMillisecondType>().value(row))
        }
        DataType::Timestamp(TimeUnit::Microsecond, _) => {
            serde_json::json!(array.as_primitive::<TimestampMicrosecondType>().value(row))
        }
        DataType::Timestamp(TimeUnit::Nanosecond, _) => {
            serde_json::json!(array.as_primitive::<TimestampNanosecondType>().value(row))
        }
        DataType::List(_) => list_value(array.as_list::<i32>().value(row))?,
        DataType::LargeList(_) => list_value(array.as_list::<i64>().value(row))?,
        DataType::ListView(_) => list_value(array.as_list_view::<i32>().value(row))?,
        DataType::LargeListView(_) => list_value(array.as_list_view::<i64>().value(row))?,
        DataType::FixedSizeList(_, _) => list_value(array.as_fixed_size_list().value(row))?,
        DataType::Struct(_) => struct_value(array.as_struct(), row)?,
        DataType::Map(_, _) => map_value(array.as_map().value(row))?,
        DataType::Dictionary(_, _) => {
            let dictionary = array.as_any_dictionary();
            let index = dictionary
                .normalized_keys()
                .get(row)
                .copied()
                .ok_or_else(|| unsupported_type(array.data_type()))?;
            array_value_to_json(dictionary.values(), index)?
        }
        other => return Err(unsupported_type(other)),
    };
    Ok(value)
}

fn list_value(values: ArrayRef) -> ResourceResult<Value> {
    (0..values.len())
        .map(|row| array_value_to_json(&values, row))
        .collect::<ResourceResult<Vec<_>>>()
        .map(Value::Array)
}

fn struct_value(array: &arrow_array::StructArray, row: usize) -> ResourceResult<Value> {
    array
        .fields()
        .iter()
        .enumerate()
        .map(|(column, field)| {
            array_value_to_json(array.column(column), row)
                .map(|value| (field.name().to_owned(), value))
        })
        .collect::<ResourceResult<Map<String, Value>>>()
        .map(Value::Object)
}

fn map_value(array: arrow_array::StructArray) -> ResourceResult<Value> {
    let mut values = Map::new();
    let keys = array.column(0);
    let entries = array.column(1);
    for row in 0..array.len() {
        let key = array_value_to_json(keys, row)?.as_str().map(str::to_owned).ok_or_else(|| {
            ResourceError::new(
                ResourceErrorKind::Validation,
                "Parquet map keys must be strings to become a JSON object",
            )
        })?;
        if values.contains_key(&key) {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "Parquet map keys must be unique to become a JSON object",
            ));
        }
        values.insert(key, array_value_to_json(entries, row)?);
    }
    Ok(Value::Object(values))
}

fn decimal_value(array: &ArrayRef, row: usize) -> ResourceResult<Value> {
    let decimal = match array.data_type() {
        DataType::Decimal32(_, _) => array.as_primitive::<Decimal32Type>().value_as_string(row),
        DataType::Decimal64(_, _) => array.as_primitive::<Decimal64Type>().value_as_string(row),
        DataType::Decimal128(_, _) => array.as_primitive::<Decimal128Type>().value_as_string(row),
        DataType::Decimal256(_, _) => array.as_primitive::<Decimal256Type>().value_as_string(row),
        other => return Err(unsupported_type(other)),
    };
    decimal.parse::<Number>().map(Value::Number).map_err(|_| {
        ResourceError::new(
            ResourceErrorKind::Validation,
            "Parquet decimal value cannot be represented as a JSON number",
        )
    })
}

fn bytes_value(bytes: &[u8]) -> Value {
    Value::Array(bytes.iter().map(|byte| Value::Number(Number::from(*byte))).collect())
}

fn number_from_f64(value: f64) -> ResourceResult<Value> {
    Number::from_f64(value).map(Value::Number).ok_or_else(|| {
        ResourceError::new(
            ResourceErrorKind::Validation,
            "Parquet floating-point value is not a finite JSON number",
        )
    })
}

fn is_string_type(data_type: &DataType) -> bool {
    match data_type {
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => true,
        DataType::Dictionary(_, value) => is_string_type(value),
        _ => false,
    }
}

fn validate_unique_keys(values: &[Map<String, Value>], key: &str) -> ResourceResult<()> {
    let mut keys = BTreeSet::new();
    for value in values {
        let key_value = value.get(key).and_then(Value::as_str).filter(|value| !value.is_empty());
        if key_value.is_none() {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "Parquet primary keys must be non-empty strings",
            )
            .with_field(key, "must be a non-empty string"));
        }
        if !keys.insert(key_value.expect("checked above")) {
            return Err(ResourceError::new(
                ResourceErrorKind::Conflict,
                "Parquet contains duplicate primary keys",
            )
            .with_field(key, "must be unique"));
        }
    }
    Ok(())
}

fn value_contains_text(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(value) => value.to_lowercase().contains(needle),
        Value::Array(values) => values.iter().any(|value| value_contains_text(value, needle)),
        Value::Object(values) => values.values().any(|value| value_contains_text(value, needle)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn compare_values(left: &Value, right: &Value, sorts: &[ikashita_resource::Sort]) -> Ordering {
    sorts
        .iter()
        .find_map(|sort| {
            let ordering = compare_json_values(
                left.get(&sort.field).unwrap_or(&Value::Null),
                right.get(&sort.field).unwrap_or(&Value::Null),
            );
            (ordering != Ordering::Equal).then_some(match sort.direction {
                SortDirection::Ascending => ordering,
                SortDirection::Descending => ordering.reverse(),
            })
        })
        .unwrap_or(Ordering::Equal)
}

fn compare_json_values(left: &Value, right: &Value) -> Ordering {
    match (left, right) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::String(left), Value::String(right)) => left.cmp(right),
        (Value::Number(left), Value::Number(right)) => left
            .as_f64()
            .and_then(|left| right.as_f64().map(|right| left.total_cmp(&right)))
            .unwrap_or_else(|| left.to_string().cmp(&right.to_string())),
        (Value::Bool(left), Value::Bool(right)) => left.cmp(right),
        (Value::Array(left), Value::Array(right)) => left.len().cmp(&right.len()),
        (Value::Object(left), Value::Object(right)) => left.len().cmp(&right.len()),
        (left, right) => value_rank(left).cmp(&value_rank(right)),
    }
}

const fn value_rank(value: &Value) -> u8 {
    match value {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Number(_) => 2,
        Value::String(_) => 3,
        Value::Array(_) => 4,
        Value::Object(_) => 5,
    }
}

fn config_error(error: crate::DataConfigError) -> ResourceError {
    ResourceError::new(ResourceErrorKind::Validation, error.message())
}

fn io_error(operation: &str, error: std::io::Error) -> ResourceError {
    ResourceError::new(ResourceErrorKind::Unavailable, format!("could not {operation}"))
        .with_field("storage", error.kind().to_string())
}

fn parquet_error(operation: &str, error: impl std::fmt::Display) -> ResourceError {
    let _ = error;
    ResourceError::new(ResourceErrorKind::Validation, operation)
        .with_field("parquet", "invalid Parquet structure or value")
}

fn unsupported_type(data_type: &DataType) -> ResourceError {
    ResourceError::new(
        ResourceErrorKind::Validation,
        "Parquet contains a value type that cannot be represented safely as JSON",
    )
    .with_field("parquet_type", format!("{data_type:?}"))
}

#[cfg(test)]
mod tests {
    use std::{fs::File, sync::Arc};

    use arrow_array::{
        ArrayRef, BinaryArray, BooleanArray, Float64Array, Int64Array, RecordBatch, StringArray,
        StructArray,
        builder::{ListBuilder, StringBuilder},
    };
    use arrow_schema::{DataType, Field};
    use ikashita_resource::{Capability, JsonResourceProvider, ListQuery, ResourceErrorKind};
    use parquet::arrow::ArrowWriter;
    use tempfile::tempdir;

    use super::*;
    use crate::{DataFormat, DataResourceConfig, DataResourceProvider};

    fn write_fixture(path: &Path) {
        let mut tags = ListBuilder::new(StringBuilder::new());
        tags.append_value([Some("rust"), Some("data")]);
        tags.append_value([Some("parquet")]);

        let profile = StructArray::from(vec![
            (
                Arc::new(Field::new("team", DataType::Utf8, true)),
                Arc::new(StringArray::from(vec![Some("platform"), None])) as ArrayRef,
            ),
            (
                Arc::new(Field::new("active", DataType::Boolean, true)),
                Arc::new(BooleanArray::from(vec![Some(true), Some(false)])) as ArrayRef,
            ),
        ]);
        let batch = RecordBatch::try_from_iter(vec![
            ("id", Arc::new(StringArray::from(vec!["p-1", "p-2"])) as ArrayRef),
            ("active", Arc::new(BooleanArray::from(vec![true, false])) as ArrayRef),
            ("count", Arc::new(Int64Array::from(vec![Some(7), None])) as ArrayRef),
            ("score", Arc::new(Float64Array::from(vec![Some(2.5), None])) as ArrayRef),
            ("label", Arc::new(StringArray::from(vec![Some("first"), None])) as ArrayRef),
            ("bytes", Arc::new(BinaryArray::from(vec![Some(&[1_u8, 2_u8][..]), None])) as ArrayRef),
            ("tags", Arc::new(tags.finish()) as ArrayRef),
            ("profile", Arc::new(profile) as ArrayRef),
        ])
        .expect("record batch");
        let file = File::create(path).expect("fixture file");
        let mut writer = ArrowWriter::try_new(file, batch.schema(), None).expect("writer");
        writer.write(&batch).expect("write batch");
        writer.close().expect("close writer");
    }

    #[test]
    fn parquet_dispatch_reads_typed_values_and_nested_json() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("records.parquet");
        write_fixture(&path);
        let provider = DataResourceProvider::new(
            DataResourceConfig::new(&path).with_format(DataFormat::Parquet).with_key("id"),
        )
        .expect("provider");

        let schema = JsonResourceProvider::schema(&provider).expect("schema");
        assert_eq!(provider.format(), DataFormat::Parquet);
        assert!(schema.capabilities.contains(&Capability::Schema));
        assert!(schema.capabilities.contains(&Capability::List));
        assert!(schema.capabilities.contains(&Capability::Get));
        assert!(!schema.capabilities.contains(&Capability::Create));
        assert_eq!(
            schema.fields.iter().find(|field| field.name == "count").unwrap().field_type,
            FieldType::Integer
        );
        assert_eq!(
            schema.fields.iter().find(|field| field.name == "bytes").unwrap().field_type,
            FieldType::Json
        );

        let page = JsonResourceProvider::list(&provider, &ListQuery::new().with_search("platform"))
            .expect("list");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0]["count"], 7);
        assert_eq!(page.items[0]["bytes"], serde_json::json!([1, 2]));
        assert_eq!(page.items[0]["tags"], serde_json::json!(["rust", "data"]));
        assert_eq!(page.items[0]["profile"]["team"], "platform");

        let first = JsonResourceProvider::get(&provider, "p-1").expect("get").expect("record");
        assert_eq!(first["active"], true);
        assert_eq!(first["score"], 2.5);
        assert_eq!(first["profile"]["team"], "platform");
        let second = JsonResourceProvider::get(&provider, "p-2").expect("get").expect("record");
        assert_eq!(second["count"], Value::Null);
        assert_eq!(second["label"], Value::Null);
        let error =
            provider.create(serde_json::json!({"id": "p-3"})).expect_err("read-only create");
        assert_eq!(error.kind, ResourceErrorKind::CapabilityDenied);
    }

    #[test]
    fn parquet_duplicate_string_keys_are_rejected() {
        let directory = tempdir().expect("tempdir");
        let path = directory.path().join("duplicates.parquet");
        let batch = RecordBatch::try_from_iter(vec![(
            "id",
            Arc::new(StringArray::from(vec!["same", "same"])) as ArrayRef,
        )])
        .expect("record batch");
        let file = File::create(&path).expect("fixture file");
        let mut writer = ArrowWriter::try_new(file, batch.schema(), None).expect("writer");
        writer.write(&batch).expect("write batch");
        writer.close().expect("close writer");

        let error =
            DataResourceProvider::new(DataResourceConfig::new(&path)).expect_err("duplicate keys");
        assert_eq!(error.kind, ResourceErrorKind::Conflict);
    }
}
