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
