use crate::models::{PropertyMap, PropertyValue};
use std::collections::{BTreeSet, HashMap};

pub(super) type PropertyIndexKey = (String, String, String);

pub(super) fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!("{field} cannot be empty"))
    } else {
        Ok(())
    }
}

pub(super) fn validate_properties(properties: &PropertyMap) -> Result<(), String> {
    for (key, value) in properties {
        validate_non_empty("property key", key)?;
        if let PropertyValue::Float(value) = value {
            if !value.is_finite() {
                return Err(format!("property {key:?} must be finite"));
            }
        }
    }
    Ok(())
}

pub(super) fn index_properties(
    id: u64,
    properties: &PropertyMap,
    key_index: &mut HashMap<String, BTreeSet<u64>>,
    value_index: &mut HashMap<PropertyIndexKey, BTreeSet<u64>>,
) {
    for (key, value) in properties {
        key_index.entry(key.clone()).or_default().insert(id);
        value_index
            .entry(property_index_key(key, value))
            .or_default()
            .insert(id);
    }
}

pub(super) fn property_index_lookup(
    key_index: &HashMap<String, BTreeSet<u64>>,
    value_index: &HashMap<PropertyIndexKey, BTreeSet<u64>>,
    key: &str,
    value: Option<&PropertyValue>,
) -> Vec<u64> {
    let ids = match value {
        Some(value) => value_index.get(&property_index_key(key, value)),
        None => key_index.get(key),
    };
    ids.map(|ids| ids.iter().copied().collect())
        .unwrap_or_default()
}

fn property_index_key(key: &str, value: &PropertyValue) -> PropertyIndexKey {
    (
        key.to_string(),
        value.type_name().to_string(),
        value.encoded_value(),
    )
}
