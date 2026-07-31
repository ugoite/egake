//! Structured errors returned by a resource provider.

use std::{collections::BTreeMap, error::Error, fmt};

/// The stable category of a resource failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceErrorKind {
    /// The input failed provider-side validation.
    Validation,
    /// The requested record does not exist.
    NotFound,
    /// The operation conflicts with current provider state.
    Conflict,
    /// The provider does not grant the requested capability.
    CapabilityDenied,
    /// The provider is temporarily unavailable.
    Unavailable,
    /// The provider encountered an unexpected failure.
    Internal,
}

impl ResourceErrorKind {
    /// Returns the wire-stable error code for this category.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Validation => "validation_failed",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::CapabilityDenied => "capability_denied",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }
}

/// A structured provider error suitable for transport adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceError {
    /// Stable machine-readable error category.
    pub kind: ResourceErrorKind,
    /// Human-readable explanation safe to show to an operator.
    pub message: String,
    /// Optional messages keyed by input field.
    pub fields: BTreeMap<String, String>,
    /// Optional request correlation identifier assigned by an adapter.
    pub request_id: Option<String>,
}

impl ResourceError {
    /// Creates a structured error without field details or a request ID.
    #[must_use]
    pub fn new(kind: ResourceErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), fields: BTreeMap::new(), request_id: None }
    }

    /// Adds or replaces a field-level validation message.
    #[must_use]
    pub fn with_field(mut self, field: impl Into<String>, message: impl Into<String>) -> Self {
        self.fields.insert(field.into(), message.into());
        self
    }

    /// Adds a request correlation identifier.
    #[must_use]
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Returns the stable machine-readable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.kind.code()
    }
}

impl fmt::Display for ResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code(), self.message)
    }
}

impl Error for ResourceError {}

/// The result type used by Resource Contract operations.
pub type ResourceResult<T> = Result<T, ResourceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_structured_error_details() {
        let error = ResourceError::new(ResourceErrorKind::Validation, "invalid contact")
            .with_field("email", "must be a valid email")
            .with_request_id("req_123");

        assert_eq!(error.code(), "validation_failed");
        assert_eq!(error.fields["email"], "must be a valid email");
        assert_eq!(error.request_id.as_deref(), Some("req_123"));
        assert_eq!(error.to_string(), "validation_failed: invalid contact");
    }
}
