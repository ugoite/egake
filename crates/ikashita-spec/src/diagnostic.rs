//! Stable diagnostics emitted while parsing and validating an application.

use std::{cmp::Ordering, fmt, slice};

/// The severity of one application-profile diagnostic.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
    /// The definition cannot be consumed safely.
    Error,
    /// The definition can be consumed, but the author should review it.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
        })
    }
}

/// Stable machine-readable codes for profile diagnostics.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticCode {
    /// A source file could not be read.
    Io,
    /// The KDL source could not be parsed.
    KdlParse,
    /// The required KDL v2 header is absent.
    KdlHeaderMissing,
    /// The KDL header declares a version other than v2.
    KdlVersionUnsupported,
    /// The application profile version is absent.
    ProfileVersionMissing,
    /// The application profile version is not supported.
    ProfileVersionUnsupported,
    /// A required node argument or property is absent.
    MissingAttribute,
    /// An attribute or argument has the wrong value type.
    InvalidAttribute,
    /// An enum-like attribute contains an unsupported value.
    InvalidEnum,
    /// A node or child node is not part of the profile.
    UnknownNode,
    /// An attribute is not part of the node's profile shape.
    UnknownAttribute,
    /// A node contains an invalid number of positional arguments.
    InvalidArguments,
    /// A name or ID is declared more than once.
    DuplicateName,
    /// A component binding is malformed or points at an invalid scope.
    InvalidBinding,
    /// A state reference does not resolve.
    UnknownState,
    /// A resource reference does not resolve.
    UnknownResource,
    /// An action reference does not resolve.
    UnknownAction,
    /// A state value cannot be represented as JSON.
    InvalidStateValue,
}

impl DiagnosticCode {
    /// Returns the stable CLI-oriented code string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Io => "IK1000",
            Self::KdlParse => "IK1001",
            Self::KdlHeaderMissing => "IK1002",
            Self::KdlVersionUnsupported => "IK1003",
            Self::ProfileVersionMissing => "IK1004",
            Self::ProfileVersionUnsupported => "IK1005",
            Self::MissingAttribute => "IK2001",
            Self::InvalidAttribute => "IK2002",
            Self::InvalidEnum => "IK2003",
            Self::UnknownNode => "IK2004",
            Self::UnknownAttribute => "IK2005",
            Self::InvalidArguments => "IK2006",
            Self::DuplicateName => "IK2007",
            Self::InvalidBinding => "IK2101",
            Self::UnknownState => "IK2102",
            Self::UnknownResource => "IK2103",
            Self::UnknownAction => "IK2104",
            Self::InvalidStateValue => "IK2201",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A one-based source location suitable for a CLI diagnostic.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceLocation {
    /// The source file, when parsing a file or a named source.
    pub file: Option<String>,
    /// One-based line number.
    pub line: usize,
    /// One-based character column.
    pub column: usize,
}

impl SourceLocation {
    /// Creates a source location.
    #[must_use]
    pub fn new(file: Option<String>, line: usize, column: usize) -> Self {
        Self { file, line, column }
    }
}

/// One deterministic, renderable profile diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    /// Stable machine-readable diagnostic code.
    pub code: DiagnosticCode,
    /// Diagnostic severity.
    pub severity: Severity,
    /// Human-readable explanation.
    pub message: String,
    /// Optional source location.
    pub location: Option<SourceLocation>,
}

impl Diagnostic {
    /// Creates a diagnostic without a source location.
    #[must_use]
    pub fn new(code: DiagnosticCode, severity: Severity, message: impl Into<String>) -> Self {
        Self { code, severity, message: message.into(), location: None }
    }

    /// Attaches a source location to this diagnostic.
    #[must_use]
    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}: ", self.severity, self.code)?;
        if let Some(location) = &self.location {
            write!(
                formatter,
                "{}:{}:{}: ",
                location.file.as_deref().unwrap_or("<input>"),
                location.line,
                location.column
            )?;
        }
        formatter.write_str(&self.message)
    }
}

/// A collection of diagnostics with stable source order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
}

impl Diagnostics {
    /// Creates an empty diagnostic collection.
    #[must_use]
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Returns whether no diagnostics have been recorded.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the number of diagnostics.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns whether at least one error is present.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|item| item.severity == Severity::Error)
    }

    /// Returns diagnostics in deterministic order.
    #[must_use]
    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.items
    }

    /// Iterates over diagnostics in deterministic order.
    pub fn iter(&self) -> slice::Iter<'_, Diagnostic> {
        self.items.iter()
    }

    /// Renders all diagnostics as newline-separated CLI lines.
    #[must_use]
    pub fn render(&self) -> String {
        self.to_string()
    }

    pub(crate) fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    pub(crate) fn sort_deterministic(&mut self) {
        self.items.sort_by(|left, right| {
            let left_location = left.location.as_ref();
            let right_location = right.location.as_ref();
            match (left_location, right_location) {
                (Some(left), Some(right)) => left.cmp(right),
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            }
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.severity.cmp(&right.severity))
            .then_with(|| left.message.cmp(&right.message))
        });
    }
}

impl fmt::Display for Diagnostics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.items.iter().enumerate() {
            if index > 0 {
                formatter.write_str("\n")?;
            }
            diagnostic.fmt(formatter)?;
        }
        Ok(())
    }
}

impl<'a> IntoIterator for &'a Diagnostics {
    type Item = &'a Diagnostic;
    type IntoIter = slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
