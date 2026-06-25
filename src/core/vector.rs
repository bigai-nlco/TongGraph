use super::properties::{validate_non_empty, validate_properties};
use super::GraphCore;
use crate::models::{PropertyMap, VectorIndexDefinition, VectorRecord, VectorSearchResult};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub(crate) struct VectorSearchOptions {
    pub(crate) labels: Vec<String>,
    pub(crate) edge_type: Option<String>,
    pub(crate) properties: PropertyMap,
    pub(crate) min_score: Option<f64>,
    pub(crate) limit: usize,
    pub(crate) offset: usize,
}

impl GraphCore {
    pub(crate) fn create_vector_index(
        &mut self,
        name: String,
        target: String,
        dimensions: usize,
        metric: String,
        model: Option<String>,
        model_version: Option<String>,
    ) -> Result<(), String> {
        let definition =
            validate_definition(name, target, dimensions, metric, model, model_version)?;
        if self.vector_indexes.contains_key(&definition.name) {
            return Err(format!("vector index {:?} already exists", definition.name));
        }
        if let Some(store) = &self.store {
            self.ensure_store_current()?;
            store.create_vector_index(&definition)?;
        }
        let name = definition.name.clone();
        self.vector_indexes.insert(name.clone(), definition);
        self.vectors.entry(name).or_default();
        self.mutation_version = self.mutation_version.wrapping_add(1);
        if self.store.is_some() {
            self.refresh_store_op_seq()?;
        }
        Ok(())
    }

    pub(crate) fn drop_vector_index(&mut self, name: &str) -> Result<(), String> {
        if !self.vector_indexes.contains_key(name) {
            return Err(format!("vector index {name:?} not found"));
        }
        if let Some(store) = &self.store {
            self.ensure_store_current()?;
            store.drop_vector_index(name)?;
        }
        self.vector_indexes.remove(name);
        self.vectors.remove(name);
        self.mutation_version = self.mutation_version.wrapping_add(1);
        if self.store.is_some() {
            self.refresh_store_op_seq()?;
        }
        Ok(())
    }

    pub(crate) fn vector_indexes(&self) -> Vec<VectorIndexDefinition> {
        let mut definitions = self.vector_indexes.values().cloned().collect::<Vec<_>>();
        definitions.sort_by(|left, right| left.name.cmp(&right.name));
        definitions
    }

    pub(crate) fn upsert_vector(
        &mut self,
        index_name: &str,
        entity_id: u64,
        vector: Vec<f64>,
    ) -> Result<(), String> {
        self.upsert_vectors(index_name, vec![(entity_id, vector)])
    }

    pub(crate) fn upsert_vectors(
        &mut self,
        index_name: &str,
        vectors: Vec<(u64, Vec<f64>)>,
    ) -> Result<(), String> {
        let definition = self.require_vector_index(index_name)?.clone();
        let mut seen = HashSet::new();
        let mut records = Vec::with_capacity(vectors.len());
        for (entity_id, vector) in vectors {
            if !seen.insert(entity_id) {
                return Err(format!("duplicate vector entity id {entity_id}"));
            }
            self.require_vector_entity(&definition, entity_id)?;
            records.push(VectorRecord {
                index_name: index_name.to_string(),
                entity_id,
                vector: validate_vector(&definition, &vector, "vector")?,
            });
        }
        if records.is_empty() {
            return Ok(());
        }
        if let Some(store) = &self.store {
            self.ensure_store_current()?;
            store.upsert_vectors(index_name, &records)?;
        }
        let index_vectors = self.vectors.entry(index_name.to_string()).or_default();
        for record in records {
            index_vectors.insert(record.entity_id, record.vector);
        }
        self.mutation_version = self.mutation_version.wrapping_add(1);
        if self.store.is_some() {
            self.refresh_store_op_seq()?;
        }
        Ok(())
    }

    pub(crate) fn get_vector(
        &self,
        index_name: &str,
        entity_id: u64,
    ) -> Result<Option<Vec<f32>>, String> {
        let definition = self.require_vector_index(index_name)?;
        self.require_vector_entity(definition, entity_id)?;
        Ok(self
            .vectors
            .get(index_name)
            .and_then(|vectors| vectors.get(&entity_id))
            .cloned())
    }

    pub(crate) fn delete_vector(&mut self, index_name: &str, entity_id: u64) -> Result<(), String> {
        self.delete_vectors(index_name, vec![entity_id])
    }

    pub(crate) fn delete_vectors(
        &mut self,
        index_name: &str,
        entity_ids: Vec<u64>,
    ) -> Result<(), String> {
        let definition = self.require_vector_index(index_name)?.clone();
        let mut seen = HashSet::new();
        let mut ids = Vec::with_capacity(entity_ids.len());
        for entity_id in entity_ids {
            if seen.insert(entity_id) {
                self.require_vector_entity(&definition, entity_id)?;
                ids.push(entity_id);
            }
        }
        if ids.is_empty() {
            return Ok(());
        }
        if let Some(store) = &self.store {
            self.ensure_store_current()?;
            store.delete_vectors(index_name, &ids)?;
        }
        if let Some(vectors) = self.vectors.get_mut(index_name) {
            for entity_id in ids {
                vectors.remove(&entity_id);
            }
        }
        self.mutation_version = self.mutation_version.wrapping_add(1);
        if self.store.is_some() {
            self.refresh_store_op_seq()?;
        }
        Ok(())
    }

    pub(crate) fn search_vector(
        &self,
        index_name: &str,
        query_vector: &[f64],
        options: &VectorSearchOptions,
    ) -> Result<Vec<VectorSearchResult>, String> {
        let definition = self.require_vector_index(index_name)?;
        validate_search(definition, options)?;
        let query = validate_vector(definition, query_vector, "query vector")?;
        let empty = HashMap::new();
        let vectors = self.vectors.get(index_name).unwrap_or(&empty);
        let mut results = Vec::new();
        for (&entity_id, vector) in vectors {
            let matches = match definition.target.as_str() {
                "node" => self
                    .nodes
                    .get(entity_id as usize)
                    .and_then(Option::as_ref)
                    .is_some_and(|node| {
                        options
                            .labels
                            .iter()
                            .all(|label| node.labels.contains(label))
                            && properties_match(&node.properties, &options.properties)
                    }),
                "edge" => self
                    .edges
                    .get(entity_id as usize)
                    .and_then(Option::as_ref)
                    .is_some_and(|edge| {
                        options
                            .edge_type
                            .as_ref()
                            .is_none_or(|edge_type| edge.edge_type == *edge_type)
                            && properties_match(&edge.properties, &options.properties)
                    }),
                _ => false,
            };
            if !matches {
                continue;
            }
            let score = similarity(&definition.metric, &query, vector);
            if options.min_score.is_some_and(|minimum| score < minimum) {
                continue;
            }
            results.push(VectorSearchResult {
                kind: definition.target.clone(),
                id: entity_id,
                score,
            });
        }
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

    pub(super) fn load_vector_state(
        &mut self,
        definitions: Vec<VectorIndexDefinition>,
        records: Vec<VectorRecord>,
    ) -> Result<(), String> {
        for definition in definitions {
            let definition = validate_definition(
                definition.name,
                definition.target,
                definition.dimensions,
                definition.metric,
                definition.model,
                definition.model_version,
            )?;
            if self
                .vector_indexes
                .insert(definition.name.clone(), definition)
                .is_some()
            {
                return Err("duplicate persisted vector index".to_string());
            }
        }
        for record in records {
            let definition = self.require_vector_index(&record.index_name)?.clone();
            self.require_vector_entity(&definition, record.entity_id)?;
            if record.vector.len() != definition.dimensions
                || record.vector.iter().any(|value| !value.is_finite())
                || (definition.metric == "cosine" && vector_norm_squared(&record.vector) == 0.0)
            {
                return Err(format!(
                    "persisted vector for index {:?} entity {} is invalid",
                    record.index_name, record.entity_id
                ));
            }
            if self
                .vectors
                .entry(record.index_name.clone())
                .or_default()
                .insert(record.entity_id, record.vector)
                .is_some()
            {
                return Err(format!(
                    "duplicate persisted vector for index {:?} entity {}",
                    record.index_name, record.entity_id
                ));
            }
        }
        for name in self.vector_indexes.keys() {
            self.vectors.entry(name.clone()).or_default();
        }
        Ok(())
    }

    pub(super) fn remove_entity_vectors(&mut self, target: &str, entity_id: u64) {
        let names = self
            .vector_indexes
            .values()
            .filter(|definition| definition.target == target)
            .map(|definition| definition.name.clone())
            .collect::<Vec<_>>();
        for name in names {
            if let Some(vectors) = self.vectors.get_mut(&name) {
                vectors.remove(&entity_id);
            }
        }
    }

    fn require_vector_index(&self, name: &str) -> Result<&VectorIndexDefinition, String> {
        self.vector_indexes
            .get(name)
            .ok_or_else(|| format!("vector index {name:?} not found"))
    }

    fn require_vector_entity(
        &self,
        definition: &VectorIndexDefinition,
        entity_id: u64,
    ) -> Result<(), String> {
        let exists = match definition.target.as_str() {
            "node" => self
                .nodes
                .get(entity_id as usize)
                .is_some_and(Option::is_some),
            "edge" => self
                .edges
                .get(entity_id as usize)
                .is_some_and(Option::is_some),
            _ => false,
        };
        if exists {
            Ok(())
        } else {
            Err(format!(
                "{} {entity_id} not found for vector index {:?}",
                definition.target, definition.name
            ))
        }
    }
}

fn validate_definition(
    name: String,
    target: String,
    dimensions: usize,
    metric: String,
    model: Option<String>,
    model_version: Option<String>,
) -> Result<VectorIndexDefinition, String> {
    validate_non_empty("vector index name", &name)?;
    if !matches!(target.as_str(), "node" | "edge") {
        return Err("vector index target must be 'node' or 'edge'".to_string());
    }
    if dimensions == 0 {
        return Err("vector index dimensions must be greater than zero".to_string());
    }
    if !matches!(metric.as_str(), "cosine" | "dot" | "euclidean") {
        return Err("vector index metric must be cosine, dot, or euclidean".to_string());
    }
    if let Some(model) = &model {
        validate_non_empty("vector model", model)?;
    }
    if let Some(version) = &model_version {
        validate_non_empty("vector model_version", version)?;
        if model.is_none() {
            return Err("vector model_version requires model".to_string());
        }
    }
    Ok(VectorIndexDefinition {
        name,
        target,
        dimensions,
        metric,
        model,
        model_version,
    })
}

fn validate_vector(
    definition: &VectorIndexDefinition,
    values: &[f64],
    field: &str,
) -> Result<Vec<f32>, String> {
    if values.len() != definition.dimensions {
        return Err(format!(
            "{field} dimensions must be {}, got {}",
            definition.dimensions,
            values.len()
        ));
    }
    let mut vector = Vec::with_capacity(values.len());
    for value in values {
        if !value.is_finite() {
            return Err(format!("{field} values must be finite"));
        }
        let value = *value as f32;
        if !value.is_finite() {
            return Err(format!("{field} values must fit finite float32"));
        }
        vector.push(value);
    }
    if definition.metric == "cosine" && vector_norm_squared(&vector) == 0.0 {
        return Err(format!("{field} cannot be zero for cosine metric"));
    }
    Ok(vector)
}

fn validate_search(
    definition: &VectorIndexDefinition,
    options: &VectorSearchOptions,
) -> Result<(), String> {
    if options.limit == 0 {
        return Err("vector search limit must be greater than zero".to_string());
    }
    if options.min_score.is_some_and(|score| !score.is_finite()) {
        return Err("vector search min_score must be finite".to_string());
    }
    validate_properties(&options.properties)?;
    if definition.target == "node" && options.edge_type.is_some() {
        return Err("edge_type cannot be used with a node vector index".to_string());
    }
    if definition.target == "edge" && !options.labels.is_empty() {
        return Err("labels cannot be used with an edge vector index".to_string());
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

fn vector_norm_squared(vector: &[f32]) -> f64 {
    vector
        .iter()
        .map(|value| f64::from(*value) * f64::from(*value))
        .sum()
}

fn similarity(metric: &str, query: &[f32], candidate: &[f32]) -> f64 {
    let dot = query
        .iter()
        .zip(candidate)
        .map(|(left, right)| f64::from(*left) * f64::from(*right))
        .sum::<f64>();
    match metric {
        "cosine" => {
            dot / (vector_norm_squared(query).sqrt() * vector_norm_squared(candidate).sqrt())
        }
        "dot" => dot,
        "euclidean" => {
            let distance = query
                .iter()
                .zip(candidate)
                .map(|(left, right)| {
                    let delta = f64::from(*left) - f64::from(*right);
                    delta * delta
                })
                .sum::<f64>()
                .sqrt();
            1.0 / (1.0 + distance)
        }
        _ => unreachable!(),
    }
}
