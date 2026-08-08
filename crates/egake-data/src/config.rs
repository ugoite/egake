//! Generic local data-resource configuration and format selection.

use std::{
    error::Error,
    fmt,
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use egake_resource::ResourceSchema;

/// The local file formats supported by the data provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataFormat {
    /// Comma-separated values with a header row.
    Csv,
    /// Apache Parquet columnar data.
    Parquet,
}

impl DataFormat {
    /// Returns the stable configuration spelling for this format.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Parquet => "parquet",
        }
    }

    /// Infers a format from a supported filename extension.
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some(extension) if extension.eq_ignore_ascii_case("csv") => Some(Self::Csv),
            Some(extension) if extension.eq_ignore_ascii_case("parquet") => Some(Self::Parquet),
            _ => None,
        }
    }
}

impl fmt::Display for DataFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for DataFormat {
    type Err = DataConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "csv" => Ok(Self::Csv),
            "parquet" => Ok(Self::Parquet),
            _ => Err(DataConfigError::new("format must be one of: csv, parquet")),
        }
    }
}

/// The default stable identifier column used by generated data resources.
pub const DEFAULT_RESOURCE_KEY: &str = "id";

/// Settings needed to open a local data resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataResourceConfig {
    path: PathBuf,
    format: Option<DataFormat>,
    key: String,
    name: Option<String>,
    writable: bool,
    backup_count: u8,
    schema: Option<ResourceSchema>,
}

impl DataResourceConfig {
    /// Creates a read-only configuration with the conventional `id` key.
    ///
    /// The format is inferred from the `.csv` or `.parquet` extension unless
    /// [`Self::with_format`] supplies it explicitly.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            format: None,
            key: DEFAULT_RESOURCE_KEY.to_owned(),
            name: None,
            writable: false,
            backup_count: 0,
            schema: None,
        }
    }

    /// Sets the file format, overriding extension inference.
    #[must_use]
    pub const fn with_format(mut self, format: DataFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Replaces the primary-key column name.
    #[must_use]
    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    /// Sets the resource name advertised by the provider.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Enables or disables writes by a writable format.
    #[must_use]
    pub const fn with_writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }

    /// Sets the number of retained backups for formats that support writes.
    #[must_use]
    pub const fn with_backup_count(mut self, backup_count: u8) -> Self {
        self.backup_count = backup_count;
        self
    }

    /// Supplies field metadata derived from an external resource schema.
    #[must_use]
    pub fn with_schema(mut self, schema: ResourceSchema) -> Self {
        self.schema = Some(schema);
        self
    }

    /// Returns the configured path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the explicitly configured format, if any.
    #[must_use]
    pub const fn configured_format(&self) -> Option<DataFormat> {
        self.format
    }

    /// Resolves the explicit format or a supported filename extension.
    pub fn resolved_format(&self) -> Result<DataFormat, DataConfigError> {
        let Some(format) = self.format.or_else(|| DataFormat::from_path(&self.path)) else {
            return Err(DataConfigError::new(
                "data resource format must be specified for paths without a .csv or .parquet extension",
            ));
        };
        if let (Some(extension_format), Some(extension)) = (
            DataFormat::from_path(&self.path),
            self.path.extension().and_then(|extension| extension.to_str()),
        ) && self.format.is_some()
            && extension_format != format
        {
            return Err(DataConfigError::new(format!(
                "data resource format '{format}' conflicts with file extension '.{extension}'"
            )));
        }
        Ok(format)
    }

    /// Returns the configured primary-key column.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the configured resource name, when one was supplied.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns whether writes are requested.
    #[must_use]
    pub const fn writable(&self) -> bool {
        self.writable
    }

    /// Returns the requested number of retained backups.
    #[must_use]
    pub const fn backup_count(&self) -> u8 {
        self.backup_count
    }

    /// Returns the optional external field metadata.
    #[must_use]
    pub fn schema(&self) -> Option<&ResourceSchema> {
        self.schema.as_ref()
    }

    /// Validates settings that can be checked without touching the filesystem.
    pub fn validate(&self) -> Result<(), DataConfigError> {
        if self.path.as_os_str().is_empty() {
            return Err(DataConfigError::new("data resource path must not be empty"));
        }
        self.resolved_format()?;
        if self.key.trim().is_empty() {
            return Err(DataConfigError::new("data resource key must not be empty"));
        }
        if self.key.chars().any(char::is_control) {
            return Err(DataConfigError::new(
                "data resource key must not contain control characters",
            ));
        }
        if self.name.as_deref().is_some_and(|name| name.trim().is_empty()) {
            return Err(DataConfigError::new("data resource name must not be empty"));
        }
        if self.name.as_deref().is_some_and(|name| {
            name != name.trim()
                || name == "."
                || name == ".."
                || name
                    .chars()
                    .any(|character| character.is_control() || matches!(character, '/' | '\\'))
        }) {
            return Err(DataConfigError::new("data resource name is not a safe path segment"));
        }
        if self.path.components().any(|component| component == Component::ParentDir) {
            return Err(DataConfigError::new("data resource path traversal is not allowed"));
        }
        Ok(())
    }
}

/// A configuration error detected before a data provider opens a file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataConfigError {
    message: String,
}

impl DataConfigError {
    /// Creates a configuration error with a human-readable message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }

    /// Returns the error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for DataConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for DataConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_infer_supported_formats_safely() {
        let config = DataResourceConfig::new("data/contacts.csv");
        assert_eq!(config.resolved_format().expect("format"), DataFormat::Csv);
        assert_eq!(config.key(), DEFAULT_RESOURCE_KEY);
        assert!(!config.writable());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn explicit_format_supports_nonstandard_extensions_but_rejects_conflicts() {
        assert_eq!(
            DataResourceConfig::new("data/table.bin")
                .with_format(DataFormat::Parquet)
                .resolved_format()
                .expect("format"),
            DataFormat::Parquet
        );
        assert!(
            DataResourceConfig::new("data/table.parquet")
                .with_format(DataFormat::Csv)
                .validate()
                .is_err()
        );
    }

    #[test]
    fn invalid_key_is_rejected_without_file_io() {
        let config = DataResourceConfig::new("data/contacts.csv").with_key("  ");
        assert_eq!(
            config.validate().expect_err("blank keys are invalid").to_string(),
            "data resource key must not be empty"
        );
    }
}
