//! Numbers from the host's tool form.

use serde_json::Value;

/// A numeric parameter: the host sends an integer field as `0.0`, so both shapes count.
pub fn of(params: &Value, key: &str) -> Option<i64> {
    let value = params.get(key)?;
    value
        .as_i64()
        .or_else(|| value.as_f64().filter(|number| number.is_finite()).map(|number| number as i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_float_form_number_reads_as_an_integer() {
        assert_eq!(of(&json!({"limit_mb": 0.0}), "limit_mb"), Some(0));
        assert_eq!(of(&json!({"limit_mb": 25}), "limit_mb"), Some(25));
        assert_eq!(of(&json!({}), "limit_mb"), None);
    }
}
