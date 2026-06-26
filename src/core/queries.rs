use super::properties::property_index_lookup;
use super::GraphCore;
use crate::models::{
    EdgeRecord, EvidenceRecord, FactorRecord, NodeRecord, PropertyValue, TraceRecord,
    VariableRecord,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Instant;

#[derive(Clone, Debug)]
pub(crate) struct QuerySpec {
    pub(crate) elements: Vec<QueryElement>,
    pub(crate) returns: Option<Vec<String>>,
    pub(crate) limit: Option<usize>,
}

#[derive(Clone, Debug)]
pub(crate) enum QueryElement {
    Node(NodePattern),
    Edge(EdgePattern),
}

#[derive(Clone, Debug)]
pub(crate) struct NodePattern {
    pub(crate) alias: String,
    pub(crate) id: Option<u64>,
    pub(crate) external_id: Option<String>,
    pub(crate) labels: Vec<String>,
    pub(crate) properties: Vec<PropertyConstraint>,
}

#[derive(Clone, Debug)]
pub(crate) struct EdgePattern {
    pub(crate) alias: Option<String>,
    pub(crate) id: Option<u64>,
    pub(crate) edge_type: Option<String>,
    pub(crate) direction: QueryDirection,
    pub(crate) properties: Vec<PropertyConstraint>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueryDirection {
    Out,
    In,
    Both,
}

impl QueryDirection {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "out" | "outgoing" => Ok(Self::Out),
            "in" | "incoming" => Ok(Self::In),
            "both" | "all" => Ok(Self::Both),
            other => Err(format!(
                "query edge direction must be 'out', 'in', or 'both', got {other:?}"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PropertyConstraint {
    pub(crate) key: String,
    pub(crate) op: PropertyOperator,
    pub(crate) value: QueryValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PropertyOperator {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    In,
    Contains,
}

impl PropertyOperator {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "eq" => Ok(Self::Eq),
            "ne" => Ok(Self::Ne),
            "lt" => Ok(Self::Lt),
            "lte" => Ok(Self::Lte),
            "gt" => Ok(Self::Gt),
            "gte" => Ok(Self::Gte),
            "in" => Ok(Self::In),
            "contains" => Ok(Self::Contains),
            other => Err(format!("unknown property filter op {other:?}")),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum QueryValue {
    Scalar(PropertyValue),
    List(Vec<PropertyValue>),
}

pub(crate) type QueryRow = BTreeMap<String, u64>;

#[derive(Clone, Debug)]
pub(crate) struct QueryProfiledResult {
    pub(crate) rows: Vec<QueryRow>,
    pub(crate) profile: QueryProfile,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct QueryProfile {
    pub(crate) plan_steps: Vec<String>,
    pub(crate) chosen_anchor_index: usize,
    pub(crate) chosen_anchor_alias: String,
    pub(crate) anchor_candidates: usize,
    pub(crate) expanded_edges: usize,
    pub(crate) filtered_edges: usize,
    pub(crate) filtered_nodes: usize,
    pub(crate) alias_conflicts: usize,
    pub(crate) result_count: usize,
    pub(crate) elapsed_ns: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AliasKind {
    Node,
    Edge,
}

impl GraphCore {
    pub(crate) fn node_count(&self) -> usize {
        self.nodes.iter().flatten().count()
    }

    pub(crate) fn edge_count(&self) -> usize {
        self.edges.iter().flatten().count()
    }

    pub(crate) fn variable_count(&self) -> usize {
        self.variables.iter().flatten().count()
    }

    pub(crate) fn factor_count(&self) -> usize {
        self.factors.iter().flatten().count()
    }

    pub(crate) fn evidence_count(&self) -> usize {
        self.evidence.iter().flatten().count()
    }

    pub(crate) fn trace_count(&self) -> usize {
        self.traces.iter().flatten().count()
    }

    pub(crate) fn node_ids(&self) -> Vec<u64> {
        self.existing_node_ids()
    }

    pub(crate) fn edge_ids(&self) -> Vec<u64> {
        self.edges
            .iter()
            .enumerate()
            .filter_map(|(id, edge)| edge.as_ref().map(|_| id as u64))
            .collect()
    }

    pub(crate) fn get_node(&self, node_id: u64) -> Option<NodeRecord> {
        self.nodes.get(node_id as usize).and_then(Clone::clone)
    }

    pub(crate) fn get_edge(&self, edge_id: u64) -> Option<EdgeRecord> {
        self.edges.get(edge_id as usize).and_then(Clone::clone)
    }

    pub(crate) fn nodes(&self) -> Vec<NodeRecord> {
        self.nodes.iter().filter_map(Clone::clone).collect()
    }

    pub(crate) fn edges(&self) -> Vec<EdgeRecord> {
        self.edges.iter().filter_map(Clone::clone).collect()
    }

    pub(crate) fn get_variable(&self, variable_id: u64) -> Option<VariableRecord> {
        self.variables
            .get(variable_id as usize)
            .and_then(Clone::clone)
    }

    pub(crate) fn get_factor(&self, factor_id: u64) -> Option<FactorRecord> {
        self.factors.get(factor_id as usize).and_then(Clone::clone)
    }

    pub(crate) fn get_evidence(&self, evidence_id: u64) -> Option<EvidenceRecord> {
        self.evidence
            .get(evidence_id as usize)
            .and_then(Clone::clone)
    }

    pub(crate) fn get_trace(&self, trace_id: u64) -> Option<TraceRecord> {
        self.traces.get(trace_id as usize).and_then(Clone::clone)
    }

    pub(crate) fn get_node_id(&self, external_id: &str) -> Option<u64> {
        self.node_by_external_id.get(external_id).copied()
    }

    pub(crate) fn nodes_with_label(&self, label: &str) -> Vec<u64> {
        self.label_index
            .get(label)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    pub(crate) fn edges_by_type(&self, edge_type: &str) -> Vec<u64> {
        self.edge_type_index
            .get(edge_type)
            .map(|ids| ids.iter().copied().collect())
            .unwrap_or_default()
    }

    pub(crate) fn nodes_with_property(&self, key: &str, value: Option<&PropertyValue>) -> Vec<u64> {
        property_index_lookup(
            &self.node_property_key_index,
            &self.node_property_value_index,
            key,
            value,
        )
    }

    pub(crate) fn edges_with_property(&self, key: &str, value: Option<&PropertyValue>) -> Vec<u64> {
        property_index_lookup(
            &self.edge_property_key_index,
            &self.edge_property_value_index,
            key,
            value,
        )
    }

    pub(crate) fn query(&self, spec: &QuerySpec) -> Result<Vec<QueryRow>, String> {
        self.query_with_profile(spec).map(|result| result.rows)
    }

    pub(crate) fn query_with_profile(
        &self,
        spec: &QuerySpec,
    ) -> Result<QueryProfiledResult, String> {
        let start = Instant::now();
        let mut profile = QueryProfile::default();
        let plan = self.validate_query(spec)?;
        let anchor = self.choose_anchor(spec)?;
        let QueryElement::Node(anchor_pattern) = &spec.elements[anchor] else {
            return Err("query anchor must be a node pattern".to_string());
        };
        let candidates = self.node_candidates(anchor_pattern)?;
        profile.chosen_anchor_index = anchor;
        profile.chosen_anchor_alias = anchor_pattern.alias.clone();
        profile.anchor_candidates = candidates.len();
        profile.plan_steps = vec![
            "validate path pattern".to_string(),
            format!("choose node anchor {}", anchor_pattern.alias),
            "expand left from anchor".to_string(),
            "expand right from anchor".to_string(),
            "project requested aliases".to_string(),
        ];

        let mut rows = Vec::new();
        for node_id in candidates {
            let mut bindings = QueryRow::new();
            if !bind_alias(&mut bindings, &anchor_pattern.alias, node_id) {
                profile.alias_conflicts += 1;
                continue;
            }

            let mut left_bindings = Vec::new();
            self.expand_left(
                spec,
                anchor,
                node_id,
                bindings,
                &mut left_bindings,
                &mut profile,
            )?;
            for binding in left_bindings {
                let Some(&anchor_id) = binding.get(&anchor_pattern.alias) else {
                    continue;
                };
                self.expand_right(
                    spec,
                    anchor,
                    anchor_id,
                    binding,
                    &plan,
                    &mut rows,
                    &mut profile,
                )?;
                if spec.limit.is_some_and(|limit| rows.len() >= limit) {
                    profile.result_count = rows.len();
                    profile.elapsed_ns = start.elapsed().as_nanos();
                    return Ok(QueryProfiledResult { rows, profile });
                }
            }
        }

        profile.result_count = rows.len();
        profile.elapsed_ns = start.elapsed().as_nanos();
        Ok(QueryProfiledResult { rows, profile })
    }

    fn validate_query(&self, spec: &QuerySpec) -> Result<QueryPlan, String> {
        if spec.elements.is_empty() {
            return Err("query match must contain at least one pattern element".to_string());
        }
        if spec.elements.len() % 2 == 0 {
            return Err("query match must alternate node, edge, node".to_string());
        }

        let mut declared = HashMap::new();
        let mut default_returns = Vec::new();
        for (index, element) in spec.elements.iter().enumerate() {
            match (index % 2, element) {
                (0, QueryElement::Node(pattern)) => {
                    validate_alias("node alias", &pattern.alias)?;
                    declare_alias(
                        &mut declared,
                        &mut default_returns,
                        &pattern.alias,
                        AliasKind::Node,
                    )?;
                    validate_property_constraints(&pattern.properties)?;
                }
                (1, QueryElement::Edge(pattern)) => {
                    if let Some(alias) = &pattern.alias {
                        validate_alias("edge alias", alias)?;
                        declare_alias(&mut declared, &mut default_returns, alias, AliasKind::Edge)?;
                    }
                    validate_property_constraints(&pattern.properties)?;
                }
                _ => return Err("query match must alternate node, edge, node".to_string()),
            }
        }

        let returns = match &spec.returns {
            Some(returns) => {
                let mut seen = BTreeSet::new();
                for alias in returns {
                    validate_alias("return alias", alias)?;
                    if !declared.contains_key(alias) {
                        return Err(format!("return alias {alias:?} is not declared"));
                    }
                    if !seen.insert(alias.clone()) {
                        return Err(format!("return alias {alias:?} is duplicated"));
                    }
                }
                returns.clone()
            }
            None => default_returns,
        };

        Ok(QueryPlan { returns })
    }

    fn choose_anchor(&self, spec: &QuerySpec) -> Result<usize, String> {
        let mut best_index = 0;
        let mut best_count = usize::MAX;

        for index in (0..spec.elements.len()).step_by(2) {
            let QueryElement::Node(pattern) = &spec.elements[index] else {
                return Err("query match must alternate node, edge, node".to_string());
            };
            let count = self.node_candidates(pattern)?.len();
            if count < best_count {
                best_count = count;
                best_index = index;
            }
        }

        Ok(best_index)
    }

    fn node_candidates(&self, pattern: &NodePattern) -> Result<Vec<u64>, String> {
        let mut indexed: Option<BTreeSet<u64>> = None;

        if let Some(id) = pattern.id {
            let mut ids = BTreeSet::new();
            if self.get_node(id).is_some() {
                ids.insert(id);
            }
            indexed = Some(ids);
        }

        if let Some(external_id) = &pattern.external_id {
            let ids = self
                .get_node_id(external_id)
                .into_iter()
                .collect::<BTreeSet<_>>();
            intersect_candidates(&mut indexed, ids);
        }

        for label in &pattern.labels {
            let ids = self.nodes_with_label(label).into_iter().collect();
            intersect_candidates(&mut indexed, ids);
        }

        for constraint in &pattern.properties {
            if constraint.op == PropertyOperator::Eq {
                let QueryValue::Scalar(value) = &constraint.value else {
                    continue;
                };
                let ids = self
                    .nodes_with_property(&constraint.key, Some(value))
                    .into_iter()
                    .collect();
                intersect_candidates(&mut indexed, ids);
            }
        }

        let candidates = match indexed {
            Some(ids) => ids.into_iter().collect(),
            None => self.existing_node_ids(),
        };

        Ok(candidates
            .into_iter()
            .filter(|&node_id| {
                self.get_node(node_id)
                    .as_ref()
                    .is_some_and(|node| node_matches(pattern, node))
            })
            .collect())
    }

    fn expand_left(
        &self,
        spec: &QuerySpec,
        node_index: usize,
        current_node: u64,
        bindings: QueryRow,
        results: &mut Vec<QueryRow>,
        profile: &mut QueryProfile,
    ) -> Result<(), String> {
        if node_index == 0 {
            results.push(bindings);
            return Ok(());
        }

        let edge_index = node_index - 1;
        let left_node_index = node_index - 2;
        let QueryElement::Edge(edge_pattern) = &spec.elements[edge_index] else {
            return Err("query match must alternate node, edge, node".to_string());
        };
        let QueryElement::Node(left_pattern) = &spec.elements[left_node_index] else {
            return Err("query match must alternate node, edge, node".to_string());
        };

        for step in self.incident_edges_for_left_expansion(current_node, edge_pattern) {
            profile.expanded_edges += 1;
            if !self.edge_matches(edge_pattern, step.edge_id) {
                profile.filtered_edges += 1;
                continue;
            }
            let Some(left_node) = self.get_node(step.next_node) else {
                profile.filtered_nodes += 1;
                continue;
            };
            if !node_matches(left_pattern, &left_node) {
                profile.filtered_nodes += 1;
                continue;
            }

            let mut next_bindings = bindings.clone();
            if let Some(edge_alias) = &edge_pattern.alias {
                if !bind_alias(&mut next_bindings, edge_alias, step.edge_id) {
                    profile.alias_conflicts += 1;
                    continue;
                }
            }
            if !bind_alias(&mut next_bindings, &left_pattern.alias, step.next_node) {
                profile.alias_conflicts += 1;
                continue;
            }
            self.expand_left(
                spec,
                left_node_index,
                step.next_node,
                next_bindings,
                results,
                profile,
            )?;
        }

        Ok(())
    }

    fn expand_right(
        &self,
        spec: &QuerySpec,
        node_index: usize,
        current_node: u64,
        bindings: QueryRow,
        plan: &QueryPlan,
        rows: &mut Vec<QueryRow>,
        profile: &mut QueryProfile,
    ) -> Result<(), String> {
        if spec.limit.is_some_and(|limit| rows.len() >= limit) {
            return Ok(());
        }
        if node_index + 1 >= spec.elements.len() {
            rows.push(project_row(&bindings, &plan.returns));
            return Ok(());
        }

        let edge_index = node_index + 1;
        let right_node_index = node_index + 2;
        let QueryElement::Edge(edge_pattern) = &spec.elements[edge_index] else {
            return Err("query match must alternate node, edge, node".to_string());
        };
        let QueryElement::Node(right_pattern) = &spec.elements[right_node_index] else {
            return Err("query match must alternate node, edge, node".to_string());
        };

        for step in self.incident_edges_for_right_expansion(current_node, edge_pattern) {
            if spec.limit.is_some_and(|limit| rows.len() >= limit) {
                return Ok(());
            }
            profile.expanded_edges += 1;
            if !self.edge_matches(edge_pattern, step.edge_id) {
                profile.filtered_edges += 1;
                continue;
            }
            let Some(right_node) = self.get_node(step.next_node) else {
                profile.filtered_nodes += 1;
                continue;
            };
            if !node_matches(right_pattern, &right_node) {
                profile.filtered_nodes += 1;
                continue;
            }

            let mut next_bindings = bindings.clone();
            if let Some(edge_alias) = &edge_pattern.alias {
                if !bind_alias(&mut next_bindings, edge_alias, step.edge_id) {
                    profile.alias_conflicts += 1;
                    continue;
                }
            }
            if !bind_alias(&mut next_bindings, &right_pattern.alias, step.next_node) {
                profile.alias_conflicts += 1;
                continue;
            }
            self.expand_right(
                spec,
                right_node_index,
                step.next_node,
                next_bindings,
                plan,
                rows,
                profile,
            )?;
        }

        Ok(())
    }

    fn incident_edges_for_right_expansion(
        &self,
        node_id: u64,
        pattern: &EdgePattern,
    ) -> Vec<QueryStep> {
        match pattern.direction {
            QueryDirection::Out => self.query_steps(node_id, true),
            QueryDirection::In => self.query_steps(node_id, false),
            QueryDirection::Both => self.query_steps_both(node_id),
        }
    }

    fn incident_edges_for_left_expansion(
        &self,
        node_id: u64,
        pattern: &EdgePattern,
    ) -> Vec<QueryStep> {
        match pattern.direction {
            QueryDirection::Out => self.query_steps(node_id, false),
            QueryDirection::In => self.query_steps(node_id, true),
            QueryDirection::Both => self.query_steps_both(node_id),
        }
    }

    fn query_steps_both(&self, node_id: u64) -> Vec<QueryStep> {
        let mut steps = self.query_steps(node_id, true);
        let mut seen = steps
            .iter()
            .map(|step| step.edge_id)
            .collect::<BTreeSet<_>>();
        for step in self.query_steps(node_id, false) {
            if seen.insert(step.edge_id) {
                steps.push(step);
            }
        }
        steps
    }

    fn query_steps(&self, node_id: u64, outgoing: bool) -> Vec<QueryStep> {
        self.edge_ids_for_node(node_id, outgoing)
            .into_iter()
            .filter_map(|edge_id| {
                let edge = self.edge_record(edge_id)?;
                let next_node = if outgoing { edge.target } else { edge.source };
                Some(QueryStep { edge_id, next_node })
            })
            .collect()
    }

    fn edge_matches(&self, pattern: &EdgePattern, edge_id: u64) -> bool {
        let Some(edge) = self.edge_record(edge_id) else {
            return false;
        };
        if pattern.id.is_some_and(|id| id != edge_id) {
            return false;
        }
        if pattern
            .edge_type
            .as_ref()
            .is_some_and(|edge_type| edge.edge_type != *edge_type)
        {
            return false;
        }
        properties_match(&edge.properties, &pattern.properties)
    }
}

struct QueryPlan {
    returns: Vec<String>,
}

struct QueryStep {
    edge_id: u64,
    next_node: u64,
}

fn validate_alias(field: &str, alias: &str) -> Result<(), String> {
    if alias.is_empty() {
        Err(format!("{field} cannot be empty"))
    } else {
        Ok(())
    }
}

fn declare_alias(
    declared: &mut HashMap<String, AliasKind>,
    default_returns: &mut Vec<String>,
    alias: &str,
    kind: AliasKind,
) -> Result<(), String> {
    match declared.get(alias) {
        Some(existing) if *existing != kind => Err(format!(
            "alias {alias:?} cannot refer to both nodes and edges"
        )),
        Some(_) => Ok(()),
        None => {
            declared.insert(alias.to_string(), kind);
            default_returns.push(alias.to_string());
            Ok(())
        }
    }
}

fn validate_property_constraints(constraints: &[PropertyConstraint]) -> Result<(), String> {
    for constraint in constraints {
        if constraint.key.is_empty() {
            return Err("property filter key cannot be empty".to_string());
        }
        match (&constraint.op, &constraint.value) {
            (PropertyOperator::In, QueryValue::List(_)) => {}
            (PropertyOperator::In, QueryValue::Scalar(_)) => {
                return Err("property filter op 'in' requires a list value".to_string());
            }
            (PropertyOperator::Contains, QueryValue::Scalar(PropertyValue::String(_))) => {}
            (PropertyOperator::Contains, _) => {
                return Err("property filter op 'contains' requires a string value".to_string());
            }
            (_, QueryValue::Scalar(_)) => {}
            (_, QueryValue::List(_)) => {
                return Err(format!(
                    "property filter op {:?} requires a scalar value",
                    constraint.op
                ));
            }
        }
    }
    Ok(())
}

fn intersect_candidates(target: &mut Option<BTreeSet<u64>>, ids: BTreeSet<u64>) {
    match target {
        Some(current) => {
            *current = current.intersection(&ids).copied().collect();
        }
        None => *target = Some(ids),
    }
}

fn node_matches(pattern: &NodePattern, node: &NodeRecord) -> bool {
    if pattern.id.is_some_and(|id| id != node.id) {
        return false;
    }
    if pattern
        .external_id
        .as_ref()
        .is_some_and(|external_id| node.external_id != *external_id)
    {
        return false;
    }
    if pattern
        .labels
        .iter()
        .any(|label| !node.labels.contains(label))
    {
        return false;
    }
    properties_match(&node.properties, &pattern.properties)
}

fn properties_match(
    properties: &HashMap<String, PropertyValue>,
    constraints: &[PropertyConstraint],
) -> bool {
    constraints.iter().all(|constraint| {
        properties
            .get(&constraint.key)
            .is_some_and(|actual| property_constraint_matches(actual, constraint))
    })
}

fn property_constraint_matches(actual: &PropertyValue, constraint: &PropertyConstraint) -> bool {
    match (&constraint.op, &constraint.value) {
        (PropertyOperator::Eq, QueryValue::Scalar(expected)) => actual == expected,
        (PropertyOperator::Ne, QueryValue::Scalar(expected)) => actual != expected,
        (PropertyOperator::Lt, QueryValue::Scalar(expected)) => {
            compare_numeric(actual, expected).is_some_and(|ordering| ordering < 0)
        }
        (PropertyOperator::Lte, QueryValue::Scalar(expected)) => {
            compare_numeric(actual, expected).is_some_and(|ordering| ordering <= 0)
        }
        (PropertyOperator::Gt, QueryValue::Scalar(expected)) => {
            compare_numeric(actual, expected).is_some_and(|ordering| ordering > 0)
        }
        (PropertyOperator::Gte, QueryValue::Scalar(expected)) => {
            compare_numeric(actual, expected).is_some_and(|ordering| ordering >= 0)
        }
        (PropertyOperator::In, QueryValue::List(values)) => {
            values.iter().any(|value| actual == value)
        }
        (PropertyOperator::Contains, QueryValue::Scalar(PropertyValue::String(expected))) => {
            matches!(actual, PropertyValue::String(actual) if actual.contains(expected))
        }
        _ => false,
    }
}

fn compare_numeric(actual: &PropertyValue, expected: &PropertyValue) -> Option<i8> {
    let actual = numeric_value(actual)?;
    let expected = numeric_value(expected)?;
    if actual < expected {
        Some(-1)
    } else if actual > expected {
        Some(1)
    } else {
        Some(0)
    }
}

fn numeric_value(value: &PropertyValue) -> Option<f64> {
    match value {
        PropertyValue::Int(value) => Some(*value as f64),
        PropertyValue::Float(value) => Some(*value),
        PropertyValue::Bool(_) | PropertyValue::String(_) => None,
    }
}

fn bind_alias(bindings: &mut QueryRow, alias: &str, id: u64) -> bool {
    match bindings.get(alias) {
        Some(existing) => *existing == id,
        None => {
            bindings.insert(alias.to_string(), id);
            true
        }
    }
}

fn project_row(bindings: &QueryRow, returns: &[String]) -> QueryRow {
    returns
        .iter()
        .filter_map(|alias| bindings.get(alias).map(|id| (alias.clone(), *id)))
        .collect()
}
