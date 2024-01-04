//! Types for responding to autocomplete interactions

use serenity::builder::AutocompleteChoice;

/// A single completion list entry
#[derive(Debug)]
pub struct Completion {
    /// The friendly name of this entry
    pub name: String,
    /// The value to be completed by this entry
    pub value: CompletionValue,
}

// TODO: a lot of build() methods can probably be changed to [try_]from() now
impl From<Completion> for AutocompleteChoice {
    #[inline]
    fn from(value: Completion) -> Self {
        let Completion { name, value } = value;
        Self::new(name, value)
    }
}

/// An error arising from an invalid conversion from f64 to JSON
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("Invalid floating-point value {0:?}")]
pub struct InvalidFloatError(f64);

/// An enum of possible completion value types
#[derive(Debug)]
pub enum CompletionValue {
    /// A numeric value
    Num(serde_json::Number),
    /// A string value
    String(String),
}

impl From<i64> for CompletionValue {
    fn from(value: i64) -> Self { Self::Num(value.into()) }
}

impl From<String> for CompletionValue {
    fn from(value: String) -> Self { Self::String(value) }
}

impl From<&String> for CompletionValue {
    fn from(value: &String) -> Self { Self::String(value.into()) }
}

impl From<&str> for CompletionValue {
    fn from(value: &str) -> Self { Self::String(value.into()) }
}

impl TryFrom<f64> for CompletionValue {
    type Error = InvalidFloatError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        serde_json::Number::from_f64(value)
            .ok_or(InvalidFloatError(value))
            .map(Self::Num)
    }
}

impl From<CompletionValue> for serde_json::Value {
    fn from(value: CompletionValue) -> Self {
        match value {
            CompletionValue::Num(n) => serde_json::Value::Number(n),
            CompletionValue::String(s) => serde_json::Value::String(s),
        }
    }
}
