//! Self-contained static bundle representation.

use std::collections::BTreeMap;

/// Static files produced by a validated application build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticBundle {
    index_html: String,
    assets: BTreeMap<String, Vec<u8>>,
}

impl StaticBundle {
    /// Creates a bundle with an HTML entry point and no additional assets.
    #[must_use]
    pub fn new(index_html: impl Into<String>) -> Self {
        Self { index_html: index_html.into(), assets: BTreeMap::new() }
    }

    /// Adds or replaces a named asset in the bundle.
    pub fn insert_asset(&mut self, name: impl Into<String>, contents: impl Into<Vec<u8>>) {
        let name = name.into();
        if is_safe_asset_name(&name) {
            self.assets.insert(name, contents.into());
        }
    }

    /// Returns the HTML entry point.
    #[must_use]
    pub fn index_html(&self) -> &str {
        &self.index_html
    }

    /// Returns all assets in deterministic name order.
    #[must_use]
    pub fn assets(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.assets
    }
}

fn is_safe_asset_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.chars().any(char::is_control)
        && name
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assets_are_available_in_stable_order() {
        let mut bundle = StaticBundle::new("<html></html>");
        bundle.insert_asset("z.js", b"z".to_vec());
        bundle.insert_asset("a.js", b"a".to_vec());

        assert_eq!(bundle.index_html(), "<html></html>");
        assert_eq!(bundle.assets().keys().next().map(String::as_str), Some("a.js"));
    }

    #[test]
    fn asset_names_cannot_escape_the_bundle_namespace() {
        let mut bundle = StaticBundle::new("<html></html>");
        bundle.insert_asset("../secret.txt", b"secret".to_vec());
        bundle.insert_asset("/absolute.txt", b"secret".to_vec());
        bundle.insert_asset("nested/app.js", b"safe".to_vec());

        assert!(!bundle.assets().contains_key("../secret.txt"));
        assert!(!bundle.assets().contains_key("/absolute.txt"));
        assert_eq!(bundle.assets().get("nested/app.js"), Some(&b"safe".to_vec()));
    }
}
