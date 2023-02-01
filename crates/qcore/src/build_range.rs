//! Helper trait for constructing a closed range

use std::ops::RangeInclusive;

/// A trait representing a closed range, with optional bounds on either end
pub trait BuildRange<T> {
    /// Convert this range into a [`RangeInclusive`] with optional bounds
    fn build_range(self) -> RangeInclusive<Option<T>>;
}

impl<T> BuildRange<T> for std::ops::RangeFull {
    fn build_range(self) -> RangeInclusive<Option<T>> { None..=None }
}

impl<T> BuildRange<T> for std::ops::RangeFrom<T> {
    fn build_range(self) -> RangeInclusive<Option<T>> { Some(self.start)..=None }
}

impl<T> BuildRange<T> for std::ops::RangeToInclusive<T> {
    fn build_range(self) -> RangeInclusive<Option<T>> { None..=Some(self.end) }
}

impl<T> BuildRange<T> for RangeInclusive<T> {
    fn build_range(self) -> RangeInclusive<Option<T>> {
        let (start, end) = self.into_inner();
        Some(start)..=Some(end)
    }
}
