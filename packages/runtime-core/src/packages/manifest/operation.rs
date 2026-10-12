//! The SDK's finite operation-schema contract, checked without executing code.
//! Matching has an additional installation-profile work budget. It cannot grant
//! capabilities or prove that a declared runtime validator exists in an entry.
use crate::packages::PackageError;
use serde_json::Value;
use std::collections::BTreeSet;

const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_NODES: usize = 100_000;
const MAX_STRING_UNITS: usize = 1_048_576;
const MAX_SCHEMA_UNITS: usize = 65_536;
const MAX_MATCH_STEPS: usize = 1_000_000;
const SCHEMA_KEYS: &[&str] = &[
    "type",
    "description",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "enum",
    "const",
    "default",
    "anyOf",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "minItems",
    "maxItems",
];

struct JsonBudget {
    units: usize,
    nodes: usize,
}
impl JsonBudget {
    fn take(&mut self, units: usize) -> bool {
        let Some(remaining) = self.units.checked_sub(units) else {
            return false;
        };
        self.units = remaining;
        true
    }
    fn string(&mut self, value: &str, value_limit: bool) -> bool {
        if !self.take(2) {
            return false;
        }
        let mut length = 0;
        for character in value.chars() {
            length += character.len_utf16();
            if value_limit && length > MAX_STRING_UNITS {
                return false;
            }
            let units = match character {
                '"' | '\\' | '\u{0008}' | '\t' | '\n' | '\u{000c}' | '\r' => 2,
                '\u{0000}'..='\u{001f}' => 6,
                _ => character.len_utf16(),
            };
            if !self.take(units) {
                return false;
            }
        }
        true
    }
    fn visit(&mut self, value: &Value, depth: usize) -> bool {
        if depth > MAX_JSON_DEPTH || self.nodes == 0 {
            return false;
        }
        self.nodes -= 1;
        match value {
            Value::Null => self.take(4),
            Value::Bool(value) => self.take(if *value { 4 } else { 5 }),
            Value::Number(value) => value.as_f64().is_some_and(|value| {
                value.is_finite() && self.take(ryu_js::Buffer::new().format_finite(value).len())
            }),
            Value::String(value) => self.string(value, true),
            Value::Array(values) => {
                self.take(2)
                    && self.take(values.len().saturating_sub(1))
                    && values.iter().all(|value| self.visit(value, depth + 1))
            }
            Value::Object(values) => {
                self.take(2)
                    && self.take(values.len().saturating_sub(1))
                    && values.iter().all(|(key, value)| {
                        !matches!(key.as_str(), "__proto__" | "constructor" | "prototype")
                            && self.string(key, false)
                            && self.take(1)
                            && self.visit(value, depth + 1)
                    })
            }
        }
    }
}

/// Count the SDK's JSON.stringify UTF-16 units, including escapes, punctuation
/// and JavaScript NumberToString. The signed raw-byte budget is a separate gate.
/// serde_json has already rejected duplicate keys and invalid Unicode decoding.
pub(super) fn bounded_json(value: &Value, max_units: usize) -> bool {
    JsonBudget {
        units: max_units,
        nodes: MAX_JSON_NODES,
    }
    .visit(value, 0)
}

// Charge recursive comparisons, object checks and linear string work. This is
// an additional profile bound, not an assertion that the SDK rejects the data.
struct Fuel(usize);
impl Fuel {
    fn step(&mut self) -> Result<(), PackageError> {
        self.take(1)
    }
    fn take(&mut self, steps: usize) -> Result<(), PackageError> {
        self.0 = self
            .0
            .checked_sub(steps)
            .ok_or(PackageError::ManifestBudgetExceeded)?;
        Ok(())
    }
}

fn invalid<T>() -> Result<T, PackageError> {
    Err(PackageError::ManifestInvalid)
}
fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|number| number.is_finite())
}
fn count(value: &Value) -> bool {
    number(value)
        .is_some_and(|number| (0.0..=1_048_576.0).contains(&number) && number.fract() == 0.0)
}
fn units(value: &str) -> usize {
    value.encode_utf16().count()
}

/// This follows SDK nodeSchema + wellFormed + default.matches. It validates
/// data declarations only; it neither applies defaults nor invokes validators.
pub(super) fn validate_operation_schema(value: &Value) -> Result<(), PackageError> {
    if !bounded_json(value, MAX_SCHEMA_UNITS) {
        return invalid();
    }
    validate_node(value, &mut Fuel(MAX_MATCH_STEPS))
}

fn validate_node(schema: &Value, fuel: &mut Fuel) -> Result<(), PackageError> {
    fuel.step()?;
    let Some(object) = schema.as_object() else {
        return invalid();
    };
    if object
        .keys()
        .any(|key| !SCHEMA_KEYS.contains(&key.as_str()))
    {
        return invalid();
    }
    let kind = match object.get("type") {
        None => None,
        Some(Value::String(kind))
            if matches!(
                kind.as_str(),
                "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
            ) =>
        {
            Some(kind.as_str())
        }
        _ => return invalid(),
    };
    if object
        .get("description")
        .is_some_and(|value| value.as_str().is_none_or(|value| units(value) > 4096))
        || object
            .get("additionalProperties")
            .is_some_and(|value| !value.is_boolean())
    {
        return invalid();
    }
    if kind.is_none()
        && !object.contains_key("anyOf")
        && !object.contains_key("enum")
        && !object.contains_key("const")
    {
        return invalid();
    }
    let properties = match object.get("properties") {
        None => None,
        Some(Value::Object(properties)) => {
            if properties
                .keys()
                .any(|key| key.is_empty() || units(key) > 128)
            {
                return invalid();
            }
            Some(properties)
        }
        _ => return invalid(),
    };
    if kind == Some("object") {
        if !object.contains_key("additionalProperties") {
            return invalid();
        }
    } else if ["properties", "required", "additionalProperties"]
        .iter()
        .any(|key| object.contains_key(*key))
    {
        return invalid();
    }
    if let Some(required) = object.get("required") {
        let Some(required) = required.as_array() else {
            return invalid();
        };
        let mut seen = BTreeSet::new();
        if required.len() > 256
            || required.iter().any(|value| {
                value.as_str().is_none_or(|key| {
                    !seen.insert(key)
                        || properties.is_none_or(|properties| !properties.contains_key(key))
                })
            })
        {
            return invalid();
        }
    }
    if kind == Some("array") {
        if !object.contains_key("items") {
            return invalid();
        }
    } else if ["items", "minItems", "maxItems"]
        .iter()
        .any(|key| object.contains_key(*key))
    {
        return invalid();
    }
    if kind != Some("string")
        && ["minLength", "maxLength"]
            .iter()
            .any(|key| object.contains_key(*key))
    {
        return invalid();
    }
    if !matches!(kind, Some("number" | "integer"))
        && ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"]
            .iter()
            .any(|key| object.contains_key(*key))
    {
        return invalid();
    }
    for key in ["minLength", "maxLength", "minItems", "maxItems"] {
        if object.get(key).is_some_and(|value| !count(value)) {
            return invalid();
        }
    }
    for key in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
        if object.get(key).is_some_and(|value| number(value).is_none()) {
            return invalid();
        }
    }
    for (min, max) in [
        ("minLength", "maxLength"),
        ("minItems", "maxItems"),
        ("minimum", "maximum"),
    ] {
        if object
            .get(min)
            .and_then(number)
            .zip(object.get(max).and_then(number))
            .is_some_and(|(min, max)| min > max)
        {
            return invalid();
        }
    }
    if let Some(values) = object.get("enum") {
        let Some(values) = values.as_array() else {
            return invalid();
        };
        if values.is_empty() || values.len() > 256 {
            return invalid();
        }
        for (index, value) in values.iter().enumerate() {
            for previous in &values[..index] {
                if equal(value, previous, fuel)? {
                    return invalid();
                }
            }
        }
    }
    if let Some(properties) = properties {
        for child in properties.values() {
            validate_node(child, fuel)?;
        }
    }
    if let Some(items) = object.get("items") {
        validate_node(items, fuel)?;
    }
    if let Some(branches) = object.get("anyOf") {
        let Some(branches) = branches.as_array() else {
            return invalid();
        };
        if branches.is_empty() || branches.len() > 16 {
            return invalid();
        }
        for branch in branches {
            validate_node(branch, fuel)?;
        }
    }
    if let Some(default) = object.get("default") {
        if !matches_value(schema, default, fuel)? {
            return invalid();
        }
    }
    Ok(())
}

fn equal(left: &Value, right: &Value, fuel: &mut Fuel) -> Result<bool, PackageError> {
    fuel.step()?;
    Ok(match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Number(left), Value::Number(right)) => left.as_f64() == right.as_f64(),
        (Value::String(left), Value::String(right)) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            fuel.take(left.len())?;
            left == right
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            for (left, right) in left.iter().zip(right) {
                if !equal(left, right, fuel)? {
                    return Ok(false);
                }
            }
            true
        }
        (Value::Object(left), Value::Object(right)) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            for (key, left) in left {
                let Some(right) = right.get(key) else {
                    return Ok(false);
                };
                if !equal(left, right, fuel)? {
                    return Ok(false);
                }
            }
            true
        }
        _ => false,
    })
}

fn matches_value(schema: &Value, value: &Value, fuel: &mut Fuel) -> Result<bool, PackageError> {
    fuel.step()?;
    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array) {
        let mut matched = false;
        for branch in branches {
            if matches_value(branch, value, fuel)? {
                matched = true;
                break;
            }
        }
        if !matched {
            return Ok(false);
        }
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        let mut matched = false;
        for item in values {
            if equal(item, value, fuel)? {
                matched = true;
                break;
            }
        }
        if !matched {
            return Ok(false);
        }
    }
    if let Some(expected) = schema.get("const") {
        if !equal(expected, value, fuel)? {
            return Ok(false);
        }
    }
    Ok(match schema.get("type").and_then(Value::as_str) {
        Some("null") => value.is_null(),
        Some("boolean") => value.is_boolean(),
        Some("string") => {
            let Some(value) = value.as_str() else {
                return Ok(false);
            };
            fuel.take(value.len())?;
            let length = value.chars().count() as f64;
            length >= schema.get("minLength").and_then(number).unwrap_or(0.0)
                && length
                    <= schema
                        .get("maxLength")
                        .and_then(number)
                        .unwrap_or(f64::INFINITY)
        }
        Some(kind @ ("number" | "integer")) => number(value).is_some_and(|value| {
            (kind != "integer" || value.fract() == 0.0)
                && value
                    >= schema
                        .get("minimum")
                        .and_then(number)
                        .unwrap_or(f64::NEG_INFINITY)
                && value
                    <= schema
                        .get("maximum")
                        .and_then(number)
                        .unwrap_or(f64::INFINITY)
                && value
                    > schema
                        .get("exclusiveMinimum")
                        .and_then(number)
                        .unwrap_or(f64::NEG_INFINITY)
                && value
                    < schema
                        .get("exclusiveMaximum")
                        .and_then(number)
                        .unwrap_or(f64::INFINITY)
        }),
        Some("array") => {
            let Some(values) = value.as_array() else {
                return Ok(false);
            };
            if (values.len() as f64) < schema.get("minItems").and_then(number).unwrap_or(0.0)
                || (values.len() as f64)
                    > schema
                        .get("maxItems")
                        .and_then(number)
                        .unwrap_or(f64::INFINITY)
            {
                return Ok(false);
            }
            let items = schema.get("items").ok_or(PackageError::ManifestInvalid)?;
            for value in values {
                if !matches_value(items, value, fuel)? {
                    return Ok(false);
                }
            }
            true
        }
        Some("object") => {
            let Some(values) = value.as_object() else {
                return Ok(false);
            };
            if let Some(required) = schema.get("required").and_then(Value::as_array) {
                for key in required {
                    fuel.step()?;
                    if key.as_str().is_none_or(|key| !values.contains_key(key)) {
                        return Ok(false);
                    }
                }
            }
            let properties = schema.get("properties").and_then(Value::as_object);
            for (key, value) in values {
                fuel.step()?;
                if let Some(child) = properties.and_then(|properties| properties.get(key)) {
                    if !matches_value(child, value, fuel)? {
                        return Ok(false);
                    }
                } else if schema.get("additionalProperties") != Some(&Value::Bool(true)) {
                    return Ok(false);
                }
            }
            true
        }
        _ => true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_budget_counts_javascript_numbers_escapes_and_utf16_units() {
        for (value, expected) in [
            (json!(1.0), 1),
            (json!(-0.0), 1),
            (json!(1e-6), 8),
            (json!(1e20), 21),
            (json!(1e21), 5),
            (json!("😀"), 4),
            (json!("\u{0000}\n\"\\"), 14),
            (json!({"😀": [true, null]}), 18),
        ] {
            assert!(bounded_json(&value, expected), "{value}");
            assert!(!bounded_json(&value, expected - 1), "{value}");
        }
    }

    #[test]
    fn bounded_json_rejects_dangerous_keys_deep_and_oversized_trees() {
        for key in ["__proto__", "constructor", "prototype"] {
            assert!(!bounded_json(&json!([{key: true}]), MAX_STRING_UNITS));
        }
        let mut value = Value::Null;
        for _ in 0..32 {
            value = json!([value]);
        }
        assert!(bounded_json(&value, MAX_STRING_UNITS));
        assert!(!bounded_json(&json!([value]), MAX_STRING_UNITS));
        assert!(!bounded_json(
            &json!(vec![Value::Null; MAX_JSON_NODES]),
            MAX_STRING_UNITS
        ));
        assert!(!bounded_json(
            &json!("😀".repeat(MAX_STRING_UNITS / 2 + 1)),
            4 * MAX_STRING_UNITS
        ));
    }

    #[test]
    fn finite_operation_default_matching_and_structural_equality() {
        for schema in [
            json!({"type":"string","minLength":1,"maxLength":1,"default":"😀"}),
            json!({"type":"integer","minimum":1.0,"default":1.0}),
            json!({"const":null,"default":null}),
            json!({"enum":[{"a":1,"b":2}],"default":{"b":2.0,"a":1.0}}),
            json!({"type":"array","items":{"type":"boolean"},"default":[false,true]}),
            json!({"type":"object","additionalProperties":false,"properties":{"a":{"type":"string","default":"a"}},"default":{}}),
            json!({"anyOf":[{"type":"string"},{"type":"null"}],"default":null}),
            json!({"type":"number","minimum":1,"exclusiveMaximum":0}),
        ] {
            assert_eq!(validate_operation_schema(&schema), Ok(()), "{schema}");
        }
        for schema in [
            json!({"enum":[1,1.0]}),
            json!({"enum":[{"a":1,"b":2},{"b":2.0,"a":1.0}]}),
            json!({"type":"string","minLength":2,"default":"😀"}),
            json!({"type":"integer","default":1.5}),
            json!({"type":"number","exclusiveMinimum":1,"default":1}),
            json!({"type":"array","items":{"type":"boolean"},"default":[1]}),
            json!({"type":"object","additionalProperties":false,"properties":{"a":{"type":"string","default":"a"}},"required":["a"],"default":{}}),
            json!({"anyOf":[{"type":"object","additionalProperties":false,"properties":{"a":{"type":"string","default":"a"}},"required":["a"]}],"default":{}}),
        ] {
            assert_eq!(
                validate_operation_schema(&schema),
                Err(PackageError::ManifestInvalid),
                "{schema}"
            );
        }
    }

    #[test]
    fn schema_text_limits_count_utf16_and_value_lengths_count_code_points() {
        let property = "😀".repeat(64);
        let valid = json!({
            "type":"object", "additionalProperties":false,
            "properties":{property:{"type":"null"}},
            "description":"😀".repeat(2048)
        });
        assert_eq!(validate_operation_schema(&valid), Ok(()));
        let mut invalid_description = valid.clone();
        invalid_description["description"] = json!("😀".repeat(2049));
        assert_eq!(
            validate_operation_schema(&invalid_description),
            Err(PackageError::ManifestInvalid)
        );
        let property = "😀".repeat(65);
        let invalid_property = json!({
            "type":"object", "additionalProperties":false,
            "properties":{property:{"type":"null"}}
        });
        assert_eq!(
            validate_operation_schema(&invalid_property),
            Err(PackageError::ManifestInvalid)
        );
    }

    #[test]
    fn unsupported_schema_keywords_and_inconsistent_nodes_are_rejected() {
        for schema in [
            json!({}),
            json!({"$ref":"file:///private"}),
            json!({"type":"string","pattern":".*"}),
            json!({"type":"string","format":"email"}),
            json!({"type":"object"}),
            json!({"type":"object","additionalProperties":false,"required":["missing"]}),
            json!({"type":"object","additionalProperties":false,"properties":{"a":{"type":"null"}},"required":["a","a"]}),
            json!({"type":"array"}),
            json!({"type":"boolean","minItems":0}),
            json!({"type":"boolean","minimum":0}),
            json!({"type":"number","minLength":0}),
            json!({"type":"string","minLength":2,"maxLength":1}),
            json!({"type":"array","items":{"type":"null"},"minItems":0.5}),
            json!({"enum":[]}),
            json!({"anyOf":[]}),
            json!({"type":"object","additionalProperties":false,"properties":{"a":true}}),
        ] {
            assert_eq!(
                validate_operation_schema(&schema),
                Err(PackageError::ManifestInvalid),
                "{schema}"
            );
        }
    }

    #[test]
    fn installation_matching_budget_refuses_expensive_sdk_valid_default() {
        // This is an additional installation-profile refusal, deliberately not
        // an SDK conformance rejection. Each array element matches the last enum.
        let choices: Vec<_> = (2..=256).chain(std::iter::once(1)).collect();
        let schema = json!({
            "type":"array", "items":{"enum":choices}, "default":vec![1;30_000]
        });
        assert!(bounded_json(&schema, MAX_SCHEMA_UNITS));
        assert_eq!(
            validate_operation_schema(&schema),
            Err(PackageError::ManifestBudgetExceeded)
        );
    }
}
