//! Caching helpers for embedded asset listings.
//!
//! GPUI's `AssetSource::list` can be called repeatedly during font/icon
//! scanning. The embedded asset set never changes at runtime, so we cache the
//! filtered results per prefix to avoid re-iterating and re-allocating strings
//! on every scan.

use gpui::SharedString;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::OnceLock;

static ASSET_LIST_CACHE: OnceLock<Mutex<HashMap<String, Vec<SharedString>>>> = OnceLock::new();

/// Return the subset of `entries` whose path starts with `path`.
///
/// Results are cached per prefix, so repeated lookups only pay the filtering
/// and allocation cost once.
pub fn cached_asset_list(
    path: &str,
    entries: impl Iterator<Item = impl AsRef<str>>,
) -> Vec<SharedString> {
    let cache = ASSET_LIST_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock();
    if let Some(list) = map.get(path) {
        return list.clone();
    }

    let list: Vec<SharedString> = entries
        .filter_map(|p| {
            let p = p.as_ref();
            if p.starts_with(path) {
                Some(SharedString::from(p.to_string()))
            } else {
                None
            }
        })
        .collect();

    map.insert(path.to_string(), list.clone());
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caches_filtered_entries_per_prefix() {
        // Use unique prefixes so the global cache does not collide with other
        // tests that exercise the same helper.
        let entries = vec![
            "fonts-per-prefix/a.ttf",
            "fonts-per-prefix/b.ttf",
            "icons-per-prefix/x.svg",
        ];
        let fonts_a = cached_asset_list("fonts-per-prefix/", entries.iter().map(|s| *s));
        let fonts_b = cached_asset_list("fonts-per-prefix/", entries.iter().map(|s| *s));
        assert_eq!(fonts_a.len(), 2);
        assert_eq!(fonts_a, fonts_b);
    }

    #[test]
    fn returns_empty_for_unknown_prefix() {
        let entries = vec!["fonts-per-test/a.ttf"];
        let list = cached_asset_list("missing-per-test/", entries.iter().map(|s| *s));
        assert!(list.is_empty());
    }

    #[test]
    fn caches_different_prefixes_independently() {
        let entries = vec!["fonts-independent/a.ttf", "icons-independent/x.svg"];
        let fonts = cached_asset_list("fonts-independent/", entries.iter().map(|s| *s));
        let icons = cached_asset_list("icons-independent/", entries.iter().map(|s| *s));
        assert_eq!(fonts.len(), 1);
        assert_eq!(icons.len(), 1);
        assert_ne!(fonts, icons);
    }
}
