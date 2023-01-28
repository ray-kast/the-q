use ordered_float::OrderedFloat;
use serde_json::Value;

use super::TryFromError;

pub(super) trait TryFromValue: Sized {
    fn try_from_val(val: Value) -> Result<Self, TryFromError>;
}

impl TryFromValue for String {
    fn try_from_val(val: Value) -> Result<Self, TryFromError> {
        if let Value::String(s) = val {
            Ok(s)
        } else {
            Err(TryFromError("Cannot parse non-string value as string"))
        }
    }
}

impl TryFromValue for i64 {
    fn try_from_val(val: Value) -> Result<Self, TryFromError> {
        if let Some(i) = val.as_i64() {
            Ok(i)
        } else {
            Err(TryFromError("Cannot parse non-int value as int"))
        }
    }
}

impl TryFromValue for OrderedFloat<f64> {
    fn try_from_val(val: Value) -> Result<Self, TryFromError> {
        if let Some(f) = val.as_f64() {
            Ok(OrderedFloat(f))
        } else {
            Err(TryFromError("Cannot parse non-float value as float"))
        }
    }
}
