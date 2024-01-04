//! Helper traits for method chaining on builder objects

/// Helpers for builder-like objects
pub trait BuilderHelpers: Sized {
    /// Apply `f` to `self` if `value` is `Some`, else return `self`
    #[inline]
    #[must_use]
    fn fold_opt<T, F: FnOnce(Self, T) -> Self>(self, value: Option<T>, f: F) -> Self {
        if let Some(value) = value {
            f(self, value)
        } else {
            self
        }
    }

    /// Apply `f` to `self` for each value in `it`
    #[inline]
    #[must_use]
    fn fold_iter<I: IntoIterator, F: FnMut(Self, I::Item) -> Self>(self, it: I, f: F) -> Self {
        it.into_iter().fold(self, f)
    }
}

impl<T> BuilderHelpers for T {}

/// Convenience trait for chaining mutations on a builder-like object
pub trait BuildWith<T>: Sized {
    /// Apply the values contained within `value` to `self`
    #[must_use]
    fn build_with(self, value: T) -> Self;

    /// Apply the values contained within `value` to `self` if `value` is
    /// `Some`, else do nothing and return `self`
    #[inline]
    #[must_use]
    fn build_with_opt(self, value: Option<T>) -> Self {
        self.fold_opt(value, BuildWith::build_with)
    }

    /// Fold self by applying all values contained within each element of
    /// `values` in sequence
    #[inline]
    #[must_use]
    fn build_with_iter<I: IntoIterator<Item = T>>(self, it: I) -> Self {
        self.fold_iter(it, BuildWith::build_with)
    }
}

/// Provides the [`build_default`](BuildDefault::build_default) convenience method, which combines
/// [`Default::default`] and [`BuildWith::build_with`]
pub trait BuildDefault<B: Default + BuildWith<Self>>: Sized {
    /// Returns `B::default().build_with(self)`
    #[inline]
    fn build_default(self) -> B { B::default().build_with(self) }
}

impl<T, B: Default + BuildWith<T>> BuildDefault<B> for T {}
