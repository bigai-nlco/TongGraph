use super::properties::{validate_non_empty, validate_properties};
use super::GraphCore;
use crate::models::{FullTextIndexDefinition, FullTextSearchResult, PropertyMap, PropertyValue};
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};

#[derive(Clone, Debug)]
pub(crate) struct FullTextSearchOptions {
    pub(crate) labels: Vec<String>,
    pub(crate) edge_type: Option<String>,
    pub(crate) properties: PropertyMap,
    pub(crate) limit: usize,
    pub(crate) offset: usize,
}

impl GraphCore {
    pub(crate) fn create_fulltext_index(
        &mut self,
        name: String,
        target: String,
        properties: Vec<String>,
        tokenizer: String,
    ) -> Result<(), String> {
        let definition = validate_definition(name, target, properties, tokenizer)?;
        if self.fulltext_indexes.contains_key(&definition.name) {
            return Err(format!(
                "full-text index {:?} already exists",
                definition.name
            ));
        }
        if let Some(store) = &self.store {
            self.ensure_store_current()?;
            store.create_fulltext_index(&definition, &self.nodes(), &self.edges())?;
        }
        self.fulltext_indexes
            .insert(definition.name.clone(), definition);
        if self.store.is_some() {
            self.refresh_store_op_seq()?;
        }
        Ok(())
    }

    pub(crate) fn drop_fulltext_index(&mut self, name: &str) -> Result<(), String> {
        if !self.fulltext_indexes.contains_key(name) {
            return Err(format!("full-text index {name:?} not found"));
        }
        if let Some(store) = &self.store {
            self.ensure_store_current()?;
            store.drop_fulltext_index(name)?;
        }
        self.fulltext_indexes.remove(name);
        if self.store.is_some() {
            self.refresh_store_op_seq()?;
        }
        Ok(())
    }

    pub(crate) fn fulltext_indexes(&self) -> Vec<FullTextIndexDefinition> {
        let mut definitions = self.fulltext_indexes.values().cloned().collect::<Vec<_>>();
        definitions.sort_by(|left, right| left.name.cmp(&right.name));
        definitions
    }

    pub(crate) fn rebuild_fulltext_index(&mut self, name: Option<&str>) -> Result<(), String> {
        let definitions = match name {
            Some(name) => vec![self
                .fulltext_indexes
                .get(name)
                .cloned()
                .ok_or_else(|| format!("full-text index {name:?} not found"))?],
            None => self.fulltext_indexes(),
        };
        if let Some(store) = &self.store {
            self.ensure_store_current()?;
            store.rebuild_fulltext_indexes(&definitions, &self.nodes(), &self.edges())?;
        }
        Ok(())
    }

    pub(crate) fn search_text(
        &self,
        index_name: &str,
        query: &str,
        mode: &str,
        options: &FullTextSearchOptions,
    ) -> Result<Vec<FullTextSearchResult>, String> {
        let definition = self
            .fulltext_indexes
            .get(index_name)
            .ok_or_else(|| format!("full-text index {index_name:?} not found"))?;
        validate_search(definition, query, mode, options)?;
        let query = SearchQuery::parse(query, mode, &definition.tokenizer)?;
        let candidate_ids = if let Some(store) = &self.store {
            Some(
                store
                    .fulltext_candidates(definition, &query.fts_expression())?
                    .into_iter()
                    .collect::<BTreeSet<_>>(),
            )
        } else {
            None
        };

        let mut results = match definition.target.as_str() {
            "node" => self
                .nodes()
                .into_iter()
                .filter(|node| {
                    candidate_ids
                        .as_ref()
                        .is_none_or(|candidates| candidates.contains(&node.id))
                        && options
                            .labels
                            .iter()
                            .all(|label| node.labels.contains(label))
                        && properties_match(&node.properties, &options.properties)
                })
                .filter_map(|node| {
                    score_properties(definition, &node.properties, &query).map(
                        |(score, matched_fields)| FullTextSearchResult {
                            kind: "node".to_string(),
                            id: node.id,
                            score,
                            matched_fields,
                        },
                    )
                })
                .collect::<Vec<_>>(),
            "edge" => self
                .edges()
                .into_iter()
                .filter(|edge| {
                    candidate_ids
                        .as_ref()
                        .is_none_or(|candidates| candidates.contains(&edge.id))
                        && options
                            .edge_type
                            .as_ref()
                            .is_none_or(|edge_type| edge.edge_type == *edge_type)
                        && properties_match(&edge.properties, &options.properties)
                })
                .filter_map(|edge| {
                    score_properties(definition, &edge.properties, &query).map(
                        |(score, matched_fields)| FullTextSearchResult {
                            kind: "edge".to_string(),
                            id: edge.id,
                            score,
                            matched_fields,
                        },
                    )
                })
                .collect::<Vec<_>>(),
            _ => unreachable!(),
        };
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(results
            .into_iter()
            .skip(options.offset)
            .take(options.limit)
            .collect())
    }
}

fn validate_definition(
    name: String,
    target: String,
    properties: Vec<String>,
    tokenizer: String,
) -> Result<FullTextIndexDefinition, String> {
    validate_non_empty("full-text index name", &name)?;
    if !matches!(target.as_str(), "node" | "edge") {
        return Err("full-text index target must be 'node' or 'edge'".to_string());
    }
    if !matches!(tokenizer.as_str(), "unicode61" | "trigram") {
        return Err("full-text tokenizer must be 'unicode61' or 'trigram'".to_string());
    }
    if properties.is_empty() {
        return Err("full-text index properties cannot be empty".to_string());
    }
    let mut seen = HashSet::new();
    for property in &properties {
        validate_non_empty("full-text property", property)?;
        if property == "external_id" {
            return Err("external_id cannot be a full-text indexed property".to_string());
        }
        if !seen.insert(property.clone()) {
            return Err(format!("duplicate full-text property {property:?}"));
        }
    }
    Ok(FullTextIndexDefinition {
        name,
        target,
        properties,
        tokenizer,
    })
}

fn validate_search(
    definition: &FullTextIndexDefinition,
    query: &str,
    mode: &str,
    options: &FullTextSearchOptions,
) -> Result<(), String> {
    if query.trim().is_empty() {
        return Err("full-text query cannot be empty".to_string());
    }
    if !matches!(mode, "all" | "any" | "phrase" | "prefix") {
        return Err("full-text mode must be all, any, phrase, or prefix".to_string());
    }
    if definition.tokenizer == "trigram" && mode == "prefix" {
        return Err("prefix mode is only supported by the unicode61 tokenizer".to_string());
    }
    if options.limit == 0 {
        return Err("full-text search limit must be greater than zero".to_string());
    }
    validate_properties(&options.properties)?;
    if definition.target == "node" && options.edge_type.is_some() {
        return Err("edge_type cannot be used with a node full-text index".to_string());
    }
    if definition.target == "edge" && !options.labels.is_empty() {
        return Err("labels cannot be used with an edge full-text index".to_string());
    }
    for label in &options.labels {
        validate_non_empty("label", label)?;
    }
    if let Some(edge_type) = &options.edge_type {
        validate_non_empty("edge_type", edge_type)?;
    }
    Ok(())
}

fn properties_match(properties: &PropertyMap, filters: &PropertyMap) -> bool {
    filters
        .iter()
        .all(|(key, expected)| properties.get(key) == Some(expected))
}

#[derive(Clone, Debug)]
struct SearchQuery {
    mode: String,
    tokenizer: String,
    normalized: String,
    units: Vec<String>,
}

impl SearchQuery {
    fn parse(query: &str, mode: &str, tokenizer: &str) -> Result<Self, String> {
        let normalized = normalize_text(query);
        let units = if mode == "phrase" {
            vec![normalized.clone()]
        } else if tokenizer == "unicode61" {
            unicode_tokens(&normalized)
        } else {
            normalized
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        if units.is_empty() {
            return Err("full-text query has no searchable terms".to_string());
        }
        if tokenizer == "trigram" {
            for unit in &units {
                if unit.chars().count() < 3 {
                    return Err(
                        "trigram full-text query fragments must contain at least 3 characters"
                            .to_string(),
                    );
                }
            }
        }
        Ok(Self {
            mode: mode.to_string(),
            tokenizer: tokenizer.to_string(),
            normalized,
            units,
        })
    }

    fn fts_expression(&self) -> String {
        let separator = if self.mode == "any" { " OR " } else { " AND " };
        if self.mode == "phrase" {
            return quote_fts(&self.normalized);
        }
        self.units
            .iter()
            .map(|unit| {
                let quoted = quote_fts(unit);
                if self.mode == "prefix" {
                    format!("{quoted}*")
                } else {
                    quoted
                }
            })
            .collect::<Vec<_>>()
            .join(separator)
    }

    fn field_matches(&self, value: &str) -> (bool, usize) {
        let normalized = normalize_text(value);
        if self.mode == "phrase" {
            let matched = normalized.contains(&self.normalized);
            return (matched, usize::from(matched));
        }
        let matches = if self.tokenizer == "unicode61" {
            let tokens = unicode_tokens(&normalized);
            self.units
                .iter()
                .map(|unit| {
                    if self.mode == "prefix" {
                        tokens.iter().any(|token| token.starts_with(unit))
                    } else {
                        tokens.contains(unit)
                    }
                })
                .collect::<Vec<_>>()
        } else {
            self.units
                .iter()
                .map(|unit| normalized.contains(unit))
                .collect::<Vec<_>>()
        };
        let count = matches.iter().filter(|matched| **matched).count();
        (count > 0, count)
    }
}

fn score_properties(
    definition: &FullTextIndexDefinition,
    properties: &PropertyMap,
    query: &SearchQuery,
) -> Option<(f64, Vec<String>)> {
    let mut matched_fields = Vec::new();
    let mut unit_matches = vec![false; query.units.len()];
    for field in &definition.properties {
        let Some(PropertyValue::String(value)) = properties.get(field) else {
            continue;
        };
        let (field_match, _) = query.field_matches(value);
        if field_match {
            matched_fields.push(field.clone());
        }
        let normalized = normalize_text(value);
        for (index, unit) in query.units.iter().enumerate() {
            let matched = if query.mode == "phrase" {
                normalized.contains(&query.normalized)
            } else if query.tokenizer == "unicode61" {
                let tokens = unicode_tokens(&normalized);
                if query.mode == "prefix" {
                    tokens.iter().any(|token| token.starts_with(unit))
                } else {
                    tokens.contains(unit)
                }
            } else {
                normalized.contains(unit)
            };
            unit_matches[index] |= matched;
        }
    }
    let matched_units = unit_matches.iter().filter(|matched| **matched).count();
    let accepted = if query.mode == "all" || query.mode == "phrase" || query.mode == "prefix" {
        matched_units == query.units.len()
    } else {
        matched_units > 0
    };
    if !accepted {
        return None;
    }
    let unit_score = matched_units as f64 / query.units.len() as f64;
    let field_score = matched_fields.len() as f64 / definition.properties.len() as f64;
    Some((0.8 * unit_score + 0.2 * field_score, matched_fields))
}

fn normalize_text(value: &str) -> String {
    value
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn unicode_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn quote_fts(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
