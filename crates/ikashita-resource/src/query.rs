//! List query and pagination values.

/// The default number of records returned by a list operation.
pub const DEFAULT_PAGE_LIMIT: u64 = 50;
/// The largest page a provider should return for one request.
pub const MAX_PAGE_LIMIT: u64 = 500;

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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortDirection {
    /// Lowest values first.
    Ascending,
    /// Highest values first.
    Descending,
}

/// A field and direction used to order list results.
#[derive(Clone, Debug, Eq, PartialEq)]
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
}

/// A page of records with the effective pagination values and total count.
#[derive(Clone, Debug, Eq, PartialEq)]
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
}
