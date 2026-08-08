//! KDL Application Profile v0.1 parser, diagnostics, and typed IR.
//!
//! The crate is deliberately owned and transport-neutral: later CLI, server,
//! and renderer crates can consume [`ApplicationDefinition`] without bringing
//! in an HTTP framework or an async runtime.

pub mod diagnostic;
pub mod ir;
pub mod parser;
pub mod profile;
#[cfg(test)]
mod tests;

pub use diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Severity, SourceLocation};
pub use ir::{
    ActionDefinition, ActionStep, ActionStepKind, ApplicationDefinition, Component, ComponentKind,
    EventBinding, PageDefinition, ResourceCapability, ResourceDefinition, StateDefinition,
};
pub use profile::{
    ApplicationProfile, KDL_APPLICATION_PROFILE, MVP_PROFILE_VERSION, ProfileVersion,
};

/// Parses a KDL source string into an owned application definition.
pub fn parse(source: &str) -> Result<ApplicationDefinition, Diagnostics> {
    ApplicationDefinition::parse(source)
}

/// Parses and validates a KDL source string.
pub fn parse_and_validate(source: &str) -> Result<ApplicationDefinition, Diagnostics> {
    ApplicationDefinition::parse_and_validate(source)
}

/// Reads and parses a KDL file.
pub fn parse_file(path: impl AsRef<std::path::Path>) -> Result<ApplicationDefinition, Diagnostics> {
    ApplicationDefinition::parse_file(path)
}

/// Reads, parses, and validates a KDL file.
pub fn parse_and_validate_file(
    path: impl AsRef<std::path::Path>,
) -> Result<ApplicationDefinition, Diagnostics> {
    ApplicationDefinition::parse_and_validate_file(path)
}
