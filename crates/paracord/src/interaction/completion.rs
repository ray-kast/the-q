//! Types for responding to autocomplete interactions

/// A single completion list entry
#[derive(Debug, serde::Serialize)]
pub struct Completion {
    /// The friendly name of this entry
    pub name: String,
    /// The value to be completed by this entry
    pub value: CompletionValue,
}

/// An enum of possible completion value types
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub enum CompletionValue {
    /// An integer value
    Int(i64),
    /// A string value
    String(String),
    /// A real (decimal) numeric value
    Real(f64),
}

impl From<i64> for CompletionValue {
    fn from(value: i64) -> Self { Self::Int(value) }
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

impl From<f64> for CompletionValue {
    fn from(value: f64) -> Self { Self::Real(value) }
}

#[cfg(test)]
mod tests {
    use serenity::builder::CreateAutocompleteResponse;

    use super::Completion;

    fn assert(
        actual: &[Completion],
        expected: impl FnOnce(&mut CreateAutocompleteResponse) -> &mut CreateAutocompleteResponse,
    ) {
        let mut actual_b = CreateAutocompleteResponse::default();
        let mut expected_b = CreateAutocompleteResponse::default();
        actual_b.set_choices(serde_json::to_value(actual).unwrap());
        expected(&mut expected_b);
        assert_eq!(expected_b.0, actual_b.0);
    }

    #[test]
    fn test_serialize() {
        assert(&[], |b| b);
        assert(
            &[
                Completion {
                    name: "foo".into(),
                    value: "foo".into(),
                },
                Completion {
                    name: "bar".into(),
                    value: 1.into(),
                },
                Completion {
                    name: "baz".into(),
                    value: 1.0.into(),
                },
            ],
            |b| {
                b.add_string_choice("foo", "foo")
                    .add_int_choice("bar", 1)
                    .add_number_choice("baz", 1.0)
            },
        );
    }
}
