use crate::models::{PropertyMap, PropertyValue};
use std::collections::HashMap;

pub(crate) fn encode_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| escape_field(value))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn decode_list(encoded: &str) -> Vec<String> {
    if encoded.is_empty() {
        Vec::new()
    } else {
        encoded.lines().map(unescape_field).collect()
    }
}

pub(crate) fn encode_map(values: &PropertyMap) -> String {
    let mut keys = values.keys().collect::<Vec<_>>();
    keys.sort();
    keys.into_iter()
        .map(|key| {
            let value = values.get(key).expect("key came from map");
            format!(
                "{}\t{}\t{}",
                escape_field(key),
                value.type_name(),
                escape_field(&value.encoded_value())
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn decode_map(encoded: &str) -> Result<PropertyMap, String> {
    let mut result = HashMap::new();
    if encoded.is_empty() {
        return Ok(result);
    }
    for line in encoded.lines() {
        let mut fields = line.splitn(3, '\t');
        let Some(key) = fields.next() else {
            return Err(format!("invalid encoded property line {line:?}"));
        };
        let Some(second) = fields.next() else {
            return Err(format!("invalid encoded property line {line:?}"));
        };
        let value = match fields.next() {
            Some(value) => decode_property_value(second, &unescape_field(value))?,
            None => PropertyValue::String(unescape_field(second)),
        };
        result.insert(unescape_field(key), value);
    }
    Ok(result)
}

pub(crate) fn encode_u64_list(values: &[u64]) -> String {
    values
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn encode_f64_list(values: &[f64]) -> String {
    values
        .iter()
        .map(f64::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn decode_u64_list(encoded: &str) -> Result<Vec<u64>, String> {
    if encoded.is_empty() {
        return Ok(Vec::new());
    }
    encoded
        .split(',')
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| format!("invalid encoded u64 value {value:?}"))
        })
        .collect()
}

pub(crate) fn decode_f64_list(encoded: &str) -> Result<Vec<f64>, String> {
    if encoded.is_empty() {
        return Ok(Vec::new());
    }
    encoded
        .split(',')
        .map(|value| {
            let parsed = value
                .parse::<f64>()
                .map_err(|_| format!("invalid encoded f64 value {value:?}"))?;
            if parsed.is_finite() {
                Ok(parsed)
            } else {
                Err(format!("encoded f64 value {value:?} must be finite"))
            }
        })
        .collect()
}

pub(crate) fn decode_property_value(
    value_type: &str,
    encoded_value: &str,
) -> Result<PropertyValue, String> {
    match value_type {
        "bool" => match encoded_value {
            "true" => Ok(PropertyValue::Bool(true)),
            "false" => Ok(PropertyValue::Bool(false)),
            _ => Err(format!("invalid bool property value {encoded_value:?}")),
        },
        "int" => encoded_value
            .parse::<i64>()
            .map(PropertyValue::Int)
            .map_err(|_| format!("invalid int property value {encoded_value:?}")),
        "float" => {
            let value = encoded_value
                .parse::<f64>()
                .map_err(|_| format!("invalid float property value {encoded_value:?}"))?;
            if value.is_finite() {
                Ok(PropertyValue::Float(value))
            } else {
                Err(format!(
                    "float property value {encoded_value:?} must be finite"
                ))
            }
        }
        "string" => Ok(PropertyValue::String(encoded_value.to_string())),
        other => Err(format!("unknown property value type {other:?}")),
    }
}

fn escape_field(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn unescape_field(value: &str) -> String {
    let mut unescaped = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            unescaped.push(ch);
            continue;
        }
        match chars.next() {
            Some('\\') => unescaped.push('\\'),
            Some('n') => unescaped.push('\n'),
            Some('t') => unescaped.push('\t'),
            Some('r') => unescaped.push('\r'),
            Some(other) => {
                unescaped.push('\\');
                unescaped.push(other);
            }
            None => unescaped.push('\\'),
        }
    }
    unescaped
}
