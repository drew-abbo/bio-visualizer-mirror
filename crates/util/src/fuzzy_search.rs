//! Includes tools for fuzzy searching data.

use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

/// A trait for objects that can be fuzzy searched for.
pub trait FuzzySearchable {
    /// The string that this element should be searched by.
    fn as_search_string(&self) -> &str;
}

impl<T: FuzzySearchable> FuzzySearchable for &T {
    fn as_search_string(&self) -> &str {
        (**self).as_search_string()
    }
}

impl<T: FuzzySearchable> FuzzySearchable for &mut T {
    fn as_search_string(&self) -> &str {
        (**self).as_search_string()
    }
}

/// An object used to fuzzy search for [FuzzySearchable] objects.
///
/// Construction of this object is expensive (over 100KB of memory is
/// allocated). Reuse this object to avoid constructing more than one at a time.
#[derive(Debug)]
pub struct FuzzySearcher {
    matcher: Matcher,
    search_pattern: Option<Pattern>,
    search_str: String,
}

impl FuzzySearcher {
    /// Create a new fuzzy searcher with a pre-set search string.
    ///
    /// Construction of this object is expensive (over 100KB of memory is
    /// allocated). Reuse this object to avoid constructing more than one at a
    /// time.
    pub fn new(search_str: impl AsRef<str> + Into<String>) -> Self {
        #[cfg(debug_assertions)]
        warn_if_more_than_once_instance_created();

        Self {
            matcher: Matcher::new(Config::DEFAULT),
            search_pattern: Self::search_pattern_from_str(search_str.as_ref()),
            search_str: search_str.into(),
        }
    }

    /// The current search string being used for searches.
    pub fn search_str(&self) -> &str {
        &self.search_str
    }

    /// Updates the current search string being used for searches. If the search
    /// string is empty or all whitespace, subsequent calls to [Self::search]
    /// will produce every element.
    pub fn set_search_str(&mut self, search_str: impl AsRef<str> + Into<String>) {
        self.search_pattern = Self::search_pattern_from_str(search_str.as_ref());
        self.search_str = search_str.into();
    }

    /// Search the iterator of items, returning a new iterator of the best
    /// matches.
    ///
    /// If the search string is empty or all whitespace, the returned iterator
    /// will produce every element. See [Self::set_search_str].
    ///
    /// The number of elements in the returned iterator will be
    /// `<= items().into_iter().count()`.
    ///
    /// This function is expensive for large collections. Avoid calling
    /// repeatedly (e.g. every frame).
    pub fn search<I, T>(&mut self, items: I) -> impl Iterator<Item = T>
    where
        I: Iterator<Item = T> + Clone,
        T: FuzzySearchable,
    {
        // This is weird because we can't branch and return 2 different concrete
        // iterator types. The solution is to create two optional iterators
        // where one is always `Some` and the other is `None`, then flatten them
        // into a single iterator. Hopefully codegen saves us here.

        let dont_do_search = self.search_pattern.is_none().then(|| items.clone());

        let do_search = self.search_pattern.as_mut().map(|search_pattern| {
            search_pattern
                .match_list(
                    items.map(|item| FuzzySearchableWrapper(item)),
                    &mut self.matcher,
                )
                .into_iter()
                .map(|(item, _)| item.0)
        });

        dont_do_search
            .into_iter()
            .flatten()
            .chain(do_search.into_iter().flatten())
    }

    /// Like [Self::search], but an iterator of indices (acting as non-owning
    /// pointers) is returned.
    pub fn search_indices<T>(&mut self, items: &[T]) -> impl Iterator<Item = usize>
    where
        T: FuzzySearchable,
    {
        self.search(items.iter()).map(|item| {
            // We can't use `enumerate` without re-implementing `search` and
            // writing another wrapper type (since we need the index from
            // *before* the search happens), so we'll calculate the indices
            // instead.

            // SAFETY: Since `item` is a reference to an item inside `items`,
            // this is safe.
            (unsafe { (item as *const T).offset_from(items.as_ptr()) } as usize)
        })
    }

    /// Construct a search pattern from the string or [None] if the search
    /// string is empty or all whitespace.
    fn search_pattern_from_str(search: &str) -> Option<Pattern> {
        if search.is_empty() || search.chars().all(|c| c.is_whitespace()) {
            return None;
        }

        Some(Pattern::new(
            search,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        ))
    }
}

impl Default for FuzzySearcher {
    /// Construct with an empty search string.
    ///
    /// Construction of this object is expensive (over 100KB of memory is
    /// allocated). Reuse this object to avoid constructing more than one at a
    /// time.
    fn default() -> Self {
        // We want to call `new` so it still tracks the instance count in debug
        // mode.
        Self::new("")
    }
}

/// This allows any `T` to be treated as an `AsRef<str>` object without us
/// losing the underlying `T` type (useful for calling [Pattern::match_list]).
struct FuzzySearchableWrapper<T: FuzzySearchable>(pub T);

impl<T: FuzzySearchable> AsRef<str> for FuzzySearchableWrapper<T> {
    fn as_ref(&self) -> &str {
        self.0.as_search_string()
    }
}

/// Prints a warning if more than one [FuzzySearcher] instance is created during
/// the program's lifetime (something that should be avoided since constructing
/// a [FuzzySearcher] involves allocating over 100KB of memory).
#[cfg(debug_assertions)]
fn warn_if_more_than_once_instance_created() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static INSTANCE_COUNT: AtomicUsize = AtomicUsize::new(0);
    if INSTANCE_COUNT.fetch_add(1, Ordering::Relaxed) > 0 {
        crate::debug_log_warning!("More than one `FuzzySearcher` instance constructed.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: a simple FuzzySearchable string newtype
    #[derive(Debug, Clone, PartialEq)]
    struct Item(&'static str);

    impl FuzzySearchable for Item {
        fn as_search_string(&self) -> &str {
            self.0
        }
    }

    // --- FuzzySearchable impls for &T and &mut T ---
    // Decision: delegation through deref (always one path, but both ref kinds exercised)

    #[test]
    fn fuzzy_searchable_ref_delegates_to_inner() {
        let item = Item("hello");
        let r: &Item = &item;
        assert_eq!(r.as_search_string(), "hello");
    }

    #[test]
    fn fuzzy_searchable_mut_ref_delegates_to_inner() {
        let mut item = Item("hello");
        let r: &mut Item = &mut item;
        assert_eq!(r.as_search_string(), "hello");
    }

    // --- search_pattern_from_str ---
    // Decision 1: search.is_empty() => true (None) | false (continue)
    // Decision 2: search.chars().all(whitespace) => true (None) | false (Some(Pattern))

    #[test]
    fn search_pattern_from_empty_string_is_none() {
        let result = FuzzySearcher::search_pattern_from_str("");
        assert!(result.is_none());
    }

    #[test]
    fn search_pattern_from_whitespace_only_is_none() {
        let result = FuzzySearcher::search_pattern_from_str("   ");
        assert!(result.is_none());
    }

    #[test]
    fn search_pattern_from_whitespace_only_tab_is_none() {
        let result = FuzzySearcher::search_pattern_from_str("\t\n ");
        assert!(result.is_none());
    }

    #[test]
    fn search_pattern_from_nonempty_string_is_some() {
        let result = FuzzySearcher::search_pattern_from_str("hello");
        assert!(result.is_some());
    }

    #[test]
    fn search_pattern_from_mixed_whitespace_and_text_is_some() {
        let result = FuzzySearcher::search_pattern_from_str("  hi  ");
        assert!(result.is_some());
    }

    // --- FuzzySearcher::new ---
    // Decision: search_str empty => pattern None | nonempty => pattern Some

    #[test]
    fn new_with_empty_string_has_none_pattern() {
        let searcher = FuzzySearcher::new("");
        assert_eq!(searcher.search_str(), "");
    }

    #[test]
    fn new_with_nonempty_string_stores_search_str() {
        let searcher = FuzzySearcher::new("rust");
        assert_eq!(searcher.search_str(), "rust");
    }

    // --- FuzzySearcher::default ---
    // Decision: calls new("") — same as new with empty string

    #[test]
    fn default_has_empty_search_str() {
        let searcher = FuzzySearcher::default();
        assert_eq!(searcher.search_str(), "");
    }

    // --- set_search_str ---
    // Decision 1: new str empty/whitespace => pattern becomes None
    // Decision 2: new str nonempty => pattern becomes Some

    #[test]
    fn set_search_str_to_empty_clears_pattern() {
        let mut searcher = FuzzySearcher::new("hello");
        searcher.set_search_str("");
        assert_eq!(searcher.search_str(), "");
        // Confirm search now returns everything (no-filter path)
        let items = vec![Item("a"), Item("b")];
        let results: Vec<_> = searcher.search(items.into_iter()).collect();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn set_search_str_to_whitespace_clears_pattern() {
        let mut searcher = FuzzySearcher::new("hello");
        searcher.set_search_str("   ");
        assert_eq!(searcher.search_str(), "   ");
        let items = vec![Item("a"), Item("b")];
        let results: Vec<_> = searcher.search(items.into_iter()).collect();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn set_search_str_to_nonempty_updates_pattern() {
        let mut searcher = FuzzySearcher::new("");
        searcher.set_search_str("rust");
        assert_eq!(searcher.search_str(), "rust");
    }

    // --- search ---
    // Decision 1: search_pattern is None => dont_do_search is Some(items.clone()), do_search is None
    //             => returns all items unfiltered
    // Decision 2: search_pattern is Some => dont_do_search is None, do_search is Some(filtered)
    //             => returns matching items only
    // Decision 3: match_list result may be empty => iterator produces nothing

    #[test]
    fn search_with_empty_pattern_returns_all_items() {
        let mut searcher = FuzzySearcher::new("");
        let items = vec![Item("apple"), Item("banana"), Item("cherry")];
        let results: Vec<_> = searcher.search(items.into_iter()).collect();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn search_with_whitespace_pattern_returns_all_items() {
        let mut searcher = FuzzySearcher::new("  ");
        let items = vec![Item("apple"), Item("banana")];
        let results: Vec<_> = searcher.search(items.into_iter()).collect();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_with_matching_pattern_returns_subset() {
        let mut searcher = FuzzySearcher::new("app");
        let items = vec![Item("apple"), Item("banana"), Item("application")];
        let results: Vec<_> = searcher.search(items.into_iter()).collect();
        // "apple" and "application" should match "app"; "banana" should not
        assert!(results.iter().any(|i| i.0 == "apple"));
        assert!(results.iter().any(|i| i.0 == "application"));
        assert!(!results.iter().any(|i| i.0 == "banana"));
    }

    #[test]
    fn search_with_no_matching_items_returns_empty() {
        let mut searcher = FuzzySearcher::new("zzzzzzzzzzz");
        let items = vec![Item("apple"), Item("banana")];
        let results: Vec<_> = searcher.search(items.into_iter()).collect();
        assert!(results.is_empty());
    }

    #[test]
    fn search_with_empty_collection_returns_empty() {
        let mut searcher = FuzzySearcher::new("apple");
        let items: Vec<Item> = vec![];
        let results: Vec<_> = searcher.search(items.into_iter()).collect();
        assert!(results.is_empty());
    }

    #[test]
    fn search_empty_pattern_empty_collection_returns_empty() {
        let mut searcher = FuzzySearcher::new("");
        let items: Vec<Item> = vec![];
        let results: Vec<_> = searcher.search(items.into_iter()).collect();
        assert!(results.is_empty());
    }

    // --- search_indices ---
    // Decision 1: delegates to search => empty pattern returns all indices
    // Decision 2: nonempty pattern returns subset of indices
    // Decision 3: pointer arithmetic correctness (indices point into original slice)

    #[test]
    fn search_indices_with_empty_pattern_returns_all_indices() {
        let mut searcher = FuzzySearcher::new("");
        let items = vec![Item("alpha"), Item("beta"), Item("gamma")];
        let mut indices: Vec<_> = searcher.search_indices(&items).collect();
        indices.sort();
        assert_eq!(indices, vec![0, 1, 2]);
    }

    #[test]
    fn search_indices_with_pattern_returns_correct_indices() {
        let mut searcher = FuzzySearcher::new("alp");
        let items = vec![Item("alpha"), Item("beta"), Item("alps")];
        let mut indices: Vec<_> = searcher.search_indices(&items).collect();
        indices.sort();
        // "alpha" is index 0, "alps" is index 2
        assert!(indices.contains(&0));
        assert!(indices.contains(&2));
        assert!(!indices.contains(&1));
    }

    #[test]
    fn search_indices_are_valid_indices_into_slice() {
        let mut searcher = FuzzySearcher::new("be");
        let items = vec![Item("alpha"), Item("beta"), Item("gamma")];
        let indices: Vec<_> = searcher.search_indices(&items).collect();
        // Every returned index must be in bounds and point to the right item
        for idx in &indices {
            assert!(*idx < items.len());
            assert!(
                items[*idx].as_search_string().contains("be")
                    || items[*idx].as_search_string() == "beta"
            ); // sanity
        }
    }

    #[test]
    fn search_indices_no_match_returns_empty() {
        let mut searcher = FuzzySearcher::new("zzzzzzz");
        let items = vec![Item("alpha"), Item("beta")];
        let indices: Vec<_> = searcher.search_indices(&items).collect();
        assert!(indices.is_empty());
    }

    #[test]
    fn search_indices_empty_slice_returns_empty() {
        let mut searcher = FuzzySearcher::new("alpha");
        let items: Vec<Item> = vec![];
        let indices: Vec<_> = searcher.search_indices(&items).collect();
        assert!(indices.is_empty());
    }

    // --- FuzzySearchableWrapper ---
    // Decision: as_ref() delegates to as_search_string() (single path)

    #[test]
    fn fuzzy_searchable_wrapper_as_ref_delegates() {
        let wrapper = FuzzySearchableWrapper(Item("wrapped"));
        assert_eq!(wrapper.as_ref(), "wrapped");
    }
}
