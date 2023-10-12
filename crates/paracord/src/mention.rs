//! Types and support traits for discord mention types not supported by serenity

use std::fmt::{self, Display};

/// A timestamp that can be mentioned as part of a Discord message
#[derive(Debug, Clone)]
#[allow(missing_copy_implementations)] // in case representation changes
pub struct MentionableTimestamp { seconds_since_unix_epoch: i64 }

impl MentionableTimestamp {
    // range of ECMAscript Date type, but in seconds instead of milliseconds 
    const MAX_SECONDS: i64 =  86400 * 100_000_000;
    const MIN_SECONDS: i64 = -86400 * 100_000_000;

    /// Creates a [`MentionableTimestamp`] representing a particular number of seconds after the Unix epoch
    #[must_use]
    pub fn from_seconds_since_unix_epoch(seconds_since_unix_epoch: i64) -> Option<MentionableTimestamp> { // TODO: should be Result like TryFrom
        match seconds_since_unix_epoch {
            seconds if seconds < Self::MIN_SECONDS => None,
            seconds if seconds > Self::MAX_SECONDS => None,
            _ => Some(MentionableTimestamp { seconds_since_unix_epoch })
        }
    }

    /// Gets the number of seconds that have passed between the Unix epoch and this [`MentionableTimestamp`]. If this timestamp comes before the Unix epoch, the value is negative.
    #[must_use]
    pub fn seconds_since_unix_epoch(&self) -> i64 { self.seconds_since_unix_epoch }

    /// Creates a [Mention] which will display this timestamp in the specified style
    #[must_use]
    pub fn mention(&self, style: TimestampStyle) -> Mention { Mention::Timestamp(self.clone(), style) }
}

/// An error indicating that a value was out of range for a [`MentionableTimestamp`]
#[derive(Copy, Clone, Debug)]
pub struct OutOfMentionableRange;

impl TryFrom<serenity::model::Timestamp> for MentionableTimestamp {
    type Error = OutOfMentionableRange;

    fn try_from(value: serenity::model::Timestamp) -> Result<Self, Self::Error> {
        Self::from_seconds_since_unix_epoch(value.unix_timestamp()).ok_or(OutOfMentionableRange)
    }
}

/// A struct that represents some way to insert a timestamp into a message. Can be thought of as an extension of [`serenity::model::mention::Mention`].
// TODO: Should we have a variant that wraps a serenity mention for completeness?
#[derive(Debug, Clone)]
pub enum Mention {
    /// A timestamp to be displayed in a particular style
    Timestamp(MentionableTimestamp, TimestampStyle),
}

impl Display for Mention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mention::Timestamp(timestamp, style) => f.write_fmt(format_args!("<t:{}:{}>", timestamp.seconds_since_unix_epoch, match style {
                TimestampStyle::ShortTime => "t",
                TimestampStyle::LongTime => "T",
                TimestampStyle::ShortDate => "d",
                TimestampStyle::LongDate => "D",
                TimestampStyle::ShortDateTime => "f",
                TimestampStyle::LongDateTime => "F",
                TimestampStyle::RelativeTime => "R",
            })),
        }
    }
}

/// A style in which Discord can present timestamps in a message
#[derive(Copy, Clone, Debug)]
pub enum TimestampStyle {
    /// e.g. 16:20
    ShortTime, 
    /// e.g. 16:20:30
    LongTime,  
    /// e.g. 20/04/2021
    ShortDate,
    /// e.g. 20 April 2021
    LongDate,
    /// e.g. 20 April 2021 16:20
    ShortDateTime,
    /// e.g. Tuesday, 20 April 2021 16:20
    LongDateTime,
    /// e.g. 2 months ago
    RelativeTime
}