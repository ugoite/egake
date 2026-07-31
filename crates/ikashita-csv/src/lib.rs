//! Configuration primitives for the CSV Resource Provider boundary.
//!
//! This crate does not perform file I/O yet. It owns the validated settings so
//! the eventual standalone provider and CLI can share one configuration model
//! without coupling the Resource Contract to a storage implementation.

pub mod config;

pub use config::{CsvConfigError, CsvResourceConfig, DEFAULT_RESOURCE_KEY};
