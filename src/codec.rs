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

pub(crate) fn encode_map(values: &HashMap<String, String>) -> String {
    let mut keys = values.keys().collect::<Vec<_>>();
    keys.sort();
    keys.into_iter()
        .map(|key| {
            let value = values.get(key).expect("key came from map");
            format!("{}\t{}", escape_field(key), escape_field(value))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn decode_map(encoded: &str) -> Result<HashMap<String, String>, String> {
    let mut result = HashMap::new();
    if encoded.is_empty() {
        return Ok(result);
    }
    for line in encoded.lines() {
        let Some((key, value)) = line.split_once('\t') else {
            return Err(format!("invalid encoded property line {line:?}"));
        };
        result.insert(unescape_field(key), unescape_field(value));
    }
    Ok(result)
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
