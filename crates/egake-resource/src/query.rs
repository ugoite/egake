//! List query and pagination values.

use serde::{Deserialize, Serialize};

use crate::{ResourceError, ResourceErrorKind, ResourceResult};

/// The default number of records returned by a list operation.
pub const DEFAULT_PAGE_LIMIT: u64 = 50;
/// The largest page a provider should return for one request.
pub const MAX_PAGE_LIMIT: u64 = 500;
/// The largest encoded list query accepted by transport adapters.
pub const MAX_QUERY_BYTES: usize = 16 * 1024;

const fn normalize_limit(limit: u64) -> u64 {
    if limit == 0 {
        1
    } else if limit > MAX_PAGE_LIMIT {
        MAX_PAGE_LIMIT
    } else {
        limit
    }
}

/// The direction for one sort key.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    /// Lowest values first.
    Ascending,
    /// Highest values first.
    Descending,
}

/// A field and direction used to order list results.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Sort {
    /// Field name to sort by.
    pub field: String,
    /// Sort direction.
    pub direction: SortDirection,
}

impl Sort {
    /// Creates an ascending sort key.
    #[must_use]
    pub fn ascending(field: impl Into<String>) -> Self {
        Self { field: field.into(), direction: SortDirection::Ascending }
    }

    /// Creates a descending sort key.
    #[must_use]
    pub fn descending(field: impl Into<String>) -> Self {
        Self { field: field.into(), direction: SortDirection::Descending }
    }
}

/// A normalized offset/limit list query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListQuery {
    /// Optional provider-defined text search.
    pub search: Option<String>,
    /// Ordered sort keys.
    pub sort: Vec<Sort>,
    /// Number of matching records to skip.
    pub offset: u64,
    /// Number of records requested, capped at [`MAX_PAGE_LIMIT`].
    pub limit: u64,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl ListQuery {
    /// Creates the default query (`offset = 0`, `limit = 50`).
    #[must_use]
    pub const fn new() -> Self {
        Self { search: None, sort: Vec::new(), offset: 0, limit: DEFAULT_PAGE_LIMIT }
    }

    /// Sets the optional text search.
    #[must_use]
    pub fn with_search(mut self, search: impl Into<String>) -> Self {
        self.search = Some(search.into());
        self
    }

    /// Sets pagination and normalizes a zero or oversized limit.
    #[must_use]
    pub const fn with_pagination(mut self, offset: u64, limit: u64) -> Self {
        self.offset = offset;
        self.limit = normalize_limit(limit);
        self
    }

    /// Adds a sort key after existing keys.
    #[must_use]
    pub fn then_sort(mut self, sort: Sort) -> Self {
        self.sort.push(sort);
        self
    }

    /// Parses the standard URL query representation.
    ///
    /// The accepted keys are `q`, `sort`, `offset`, and `limit`. Sort values
    /// are comma-separated field names; a leading `-` or a `:desc` suffix
    /// requests descending order. An absent or empty sort value means no
    /// ordering constraint.
    pub fn from_query_string(query: &str) -> ResourceResult<Self> {
        if query.len() > MAX_QUERY_BYTES {
            return Err(ResourceError::new(
                ResourceErrorKind::Validation,
                "request query is too large",
            ));
        }
        let mut parsed = Self::new();
        let params: Vec<(String, String)> =
            serde_urlencoded::from_str(query).map_err(|_error| {
                ResourceError::new(ResourceErrorKind::Validation, "invalid list query")
                    .with_field("query", "invalid encoding or query pair")
            })?;

        for (key, value) in params {
            match key.as_str() {
                "q" => {
                    if !value.is_empty() {
                        parsed.search = Some(value);
                    }
                }
                "sort" => parsed.sort = parse_sort(&value)?,
                "offset" => {
                    parsed.offset = value.parse::<u64>().map_err(|_| {
                        ResourceError::new(ResourceErrorKind::Validation, "invalid list query")
                            .with_field("offset", "must be a non-negative integer")
                    })?;
                }
                "limit" => {
                    let limit = value.parse::<u64>().map_err(|_| {
                        ResourceError::new(ResourceErrorKind::Validation, "invalid list query")
                            .with_field("limit", "must be a positive integer")
                    })?;
                    parsed.limit = normalize_limit(limit);
                }
                _ => {}
            }
        }
        Ok(parsed)
    }
}

fn parse_sort(value: &str) -> ResourceResult<Vec<Sort>> {
    let mut sort = Vec::new();
    for raw_field in value.split(',').filter(|field| !field.is_empty()) {
        let (field, direction) = if let Some(field) = raw_field.strip_prefix('-') {
            (field, SortDirection::Descending)
        } else if let Some(field) = raw_field.strip_suffix(":desc") {
            (field, SortDirection::Descending)
        } else if let Some(field) = raw_field.strip_suffix(":asc") {
            (field, SortDirection::Ascending)
        } else {
            (raw_field, SortDirection::Ascending)
        };
        if field.trim().is_empty() {
            return Err(ResourceError::new(ResourceErrorKind::Validation, "invalid list query")
                .with_field("sort", "sort fields must not be empty"));
        }
        sort.push(Sort { field: field.to_owned(), direction });
    }
    Ok(sort)
}

/// A page of records with the effective pagination values and total count.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResourcePage<T> {
    /// Records returned for this page.
    pub items: Vec<T>,
    /// Total records matching the query before pagination.
    pub total: u64,
    /// Effective offset used by the provider.
    pub offset: u64,
    /// Effective limit used by the provider.
    pub limit: u64,
}

impl<T> ResourcePage<T> {
    /// Creates a page and normalizes its limit to the contract bounds.
    #[must_use]
    pub const fn new(items: Vec<T>, total: u64, offset: u64, limit: u64) -> Self {
        Self { items, total, offset, limit: normalize_limit(limit) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_uses_mvp_bounds() {
        assert_eq!(ListQuery::new().limit, DEFAULT_PAGE_LIMIT);
        assert_eq!(ListQuery::new().with_pagination(12, 0).limit, 1);
        assert_eq!(ListQuery::new().with_pagination(12, 900).limit, MAX_PAGE_LIMIT);
        assert_eq!(ListQuery::new().with_pagination(12, 25).offset, 12);
    }

    #[test]
    fn sort_keys_keep_their_order() {
        let query = ListQuery::new()
            .then_sort(Sort::ascending("name"))
            .then_sort(Sort::descending("updated_at"));

        assert_eq!(query.sort[0].field, "name");
        assert_eq!(query.sort[1].direction, SortDirection::Descending);
    }

    #[test]
    fn parses_standard_url_query_values() {
        let query = ListQuery::from_query_string(
            "q=Ada%20Lovelace&sort=-name,email:asc&offset=2&limit=900",
        )
        .expect("query");
        assert_eq!(query.search.as_deref(), Some("Ada Lovelace"));
        assert_eq!(query.sort[0], Sort::descending("name"));
        assert_eq!(query.sort[1], Sort::ascending("email"));
        assert_eq!(query.offset, 2);
        assert_eq!(query.limit, MAX_PAGE_LIMIT);
    }

    #[test]
    fn rejects_invalid_pagination_values() {
        let error = ListQuery::from_query_string("limit=nope").expect_err("invalid limit");
        assert_eq!(error.code(), "validation_failed");
        assert!(error.fields.contains_key("limit"));
    }
}
