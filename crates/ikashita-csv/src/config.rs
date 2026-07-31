//! CSV provider configuration and local validation.

use std::{
    error::Error,
    fmt,
    path::{Component, Path, PathBuf},
};

/// The default primary-key column used by generated CSV resources.
pub const DEFAULT_RESOURCE_KEY: &str = "id";

/// Settings needed to open a CSV resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvResourceConfig {
    path: PathBuf,
    key: String,
    name: Option<String>,
    writable: bool,
    backup_count: u8,
}

impl CsvResourceConfig {
    /// Creates a read-only configuration with the conventional `id` key.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            key: DEFAULT_RESOURCE_KEY.to_owned(),
            name: None,
            writable: false,
            backup_count: 0,
        }
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

    /// Enables or disables writes by the provider.
    #[must_use]
    pub const fn with_writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }

    /// Sets the number of retained backups.
    #[must_use]
    pub const fn with_backup_count(mut self, backup_count: u8) -> Self {
        self.backup_count = backup_count;
        self
    }

    /// Returns the configured CSV path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
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

    /// Returns whether writes are permitted.
    #[must_use]
    pub const fn writable(&self) -> bool {
        self.writable
    }

    /// Returns the requested number of retained backups.
    #[must_use]
    pub const fn backup_count(&self) -> u8 {
        self.backup_count
    }

    /// Validates settings that can be checked without touching the filesystem.
    pub fn validate(&self) -> Result<(), CsvConfigError> {
        if self.path.as_os_str().is_empty() {
            return Err(CsvConfigError::new("CSV path must not be empty"));
        }
        if self.key.trim().is_empty() {
            return Err(CsvConfigError::new("CSV resource key must not be empty"));
        }
        if self.name.as_deref().is_some_and(|name| name.trim().is_empty()) {
            return Err(CsvConfigError::new("CSV resource name must not be empty"));
        }
        if self.path.components().any(|component| component == Component::ParentDir) {
            return Err(CsvConfigError::new("CSV path traversal is not allowed"));
        }
        Ok(())
    }
}

/// A configuration error detected before a CSV provider opens a file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CsvConfigError {
    message: String,
}

impl CsvConfigError {
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

impl fmt::Display for CsvConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for CsvConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe_for_local_configuration() {
        let config = CsvResourceConfig::new("data/contacts.csv");

        assert_eq!(config.key(), DEFAULT_RESOURCE_KEY);
        assert!(!config.writable());
        assert_eq!(config.backup_count(), 0);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn invalid_key_is_rejected_without_file_io() {
        let config = CsvResourceConfig::new("data/contacts.csv").with_key("  ");

        assert_eq!(
            config.validate().expect_err("blank keys are invalid").to_string(),
            "CSV resource key must not be empty"
        );
    }
}
