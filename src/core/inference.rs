use super::direction::Direction;
use super::metadata::normalize;
use super::GraphCore;
use crate::models::{PropertyMap, PropertyValue};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

#[derive(Clone, Debug)]
pub(crate) struct ActiveSubgraph {
    pub(crate) variables: Vec<u64>,
    pub(crate) factors: Vec<u64>,
    pub(crate) graph_nodes: Vec<u64>,
    pub(crate) boundary_variables: Vec<u64>,
    pub(crate) truncated: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct BeliefPropagationResult {
    pub(crate) beliefs: BTreeMap<u64, BTreeMap<String, f64>>,
    pub(crate) active: ActiveSubgraph,
    pub(crate) schedule: String,
    pub(crate) iterations: usize,
    pub(crate) messages_updated: usize,
    pub(crate) converged: bool,
    pub(crate) max_residual: f64,
    pub(crate) trace_id: Option<u64>,
    pub(crate) warnings: Vec<String>,
    pub(crate) diagnostics: BTreeMap<String, PropertyValue>,
}

#[derive(Clone)]
struct MessageUpdate {
    factor_id: u64,
    variable_id: u64,
    direction: MessageDirection,
    candidate: Vec<f64>,
    residual: f64,
}

#[derive(Clone, Copy)]
enum MessageDirection {
    VariableToFactor,
    FactorToVariable,
}

impl GraphCore {
    pub(crate) fn compile_active_subgraph(
        &self,
        query_variables: &[u64],
        evidence: &HashMap<u64, String>,
        radius: usize,
        max_nodes: usize,
        max_factors: usize,
    ) -> Result<ActiveSubgraph, String> {
        let mut active_variables = BTreeSet::new();
        let mut active_nodes = BTreeSet::new();
        let mut truncated = false;

        for &variable_id in query_variables {
            self.require_variable(variable_id)?;
            active_variables.insert(variable_id);
            if let Some(owner_id) = self.variable_owner(variable_id) {
                insert_capped_node(&mut active_nodes, owner_id, max_nodes, &mut truncated);
            }
        }
        for (&variable_id, state) in evidence {
            self.require_variable(variable_id)?;
            self.state_index(variable_id, state)?;
            active_variables.insert(variable_id);
            if let Some(owner_id) = self.variable_owner(variable_id) {
                insert_capped_node(&mut active_nodes, owner_id, max_nodes, &mut truncated);
            }
        }

        if active_variables.is_empty() {
            active_variables.extend(self.existing_variable_ids());
            for owner_id in active_variables
                .iter()
                .filter_map(|&variable_id| self.variable_owner(variable_id))
            {
                insert_capped_node(&mut active_nodes, owner_id, max_nodes, &mut truncated);
            }
        }

        let seeds = active_nodes.iter().copied().collect::<Vec<_>>();
        for node_id in expand_nodes(self, &seeds, radius, max_nodes, &mut truncated)? {
            active_nodes.insert(node_id);
        }

        for variable_id in self.existing_variable_ids() {
            if let Some(owner_id) = self.variable_owner(variable_id) {
                if active_nodes.contains(&owner_id) {
                    active_variables.insert(variable_id);
                }
            }
        }

        let factor_seed_variables = active_variables.clone();
        let mut active_factors = BTreeSet::new();
        let mut factor_tables = self
            .factor_tables
            .values()
            .map(|table| (table.factor_id, table.variables.clone()))
            .collect::<Vec<_>>();
        factor_tables.sort_by_key(|(factor_id, _)| *factor_id);
        for (factor_id, variables) in factor_tables {
            if !variables
                .iter()
                .any(|variable_id| factor_seed_variables.contains(variable_id))
            {
                continue;
            }
            if active_factors.len() >= max_factors {
                truncated = true;
                continue;
            }
            active_factors.insert(factor_id);
            for variable_id in variables {
                if active_variables.insert(variable_id) {
                    if let Some(owner_id) = self.variable_owner(variable_id) {
                        insert_capped_node(&mut active_nodes, owner_id, max_nodes, &mut truncated);
                    }
                }
            }
        }

        let boundary_variables = active_variables
            .iter()
            .copied()
            .filter(|&variable_id| {
                self.variable_owner(variable_id)
                    .is_some_and(|owner_id| !active_nodes.contains(&owner_id))
            })
            .collect();

        Ok(ActiveSubgraph {
            variables: active_variables.into_iter().collect(),
            factors: active_factors.into_iter().collect(),
            graph_nodes: active_nodes.into_iter().collect(),
            boundary_variables,
            truncated,
        })
    }

    pub(crate) fn belief_propagation(
        &mut self,
        query_variables: Option<&[u64]>,
        runtime_evidence: &HashMap<u64, String>,
        radius: usize,
        max_iters: usize,
        tolerance: f64,
        damping: f64,
        persist: bool,
    ) -> Result<BeliefPropagationResult, String> {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return Err("tolerance must be finite and non-negative".to_string());
        }
        if !(0.0..=1.0).contains(&damping) {
            return Err("damping must be between 0.0 and 1.0".to_string());
        }

        let evidence = self.evidence_for_run(runtime_evidence)?;
        let query_variables = query_variables.unwrap_or(&[]);
        let active =
            self.compile_active_subgraph(query_variables, &evidence, radius, 10000, 50000)?;
        let mut warnings = active_subgraph_warnings(&active);
        let active_variable_set = active.variables.iter().copied().collect::<BTreeSet<_>>();
        let active_factor_set = active.factors.iter().copied().collect::<BTreeSet<_>>();

        let mut v_to_f = HashMap::new();
        let mut f_to_v = HashMap::new();
        for &factor_id in &active.factors {
            let table = self
                .factor_tables
                .get(&factor_id)
                .ok_or_else(|| format!("factor table {factor_id} not found"))?;
            for &variable_id in &table.variables {
                if !active_variable_set.contains(&variable_id) {
                    continue;
                }
                let uniform = uniform(self.variable_states(variable_id)?.len());
                v_to_f.insert((variable_id, factor_id), uniform.clone());
                f_to_v.insert((factor_id, variable_id), uniform);
            }
        }

        let mut messages_updated = 0usize;
        let mut iterations = 0usize;
        let mut max_residual = 0.0;
        let mut converged = active.factors.is_empty();

        for iteration in 0..max_iters {
            iterations = iteration + 1;
            let Some(update) =
                self.best_message_update(&active_factor_set, &evidence, &v_to_f, &f_to_v)?
            else {
                converged = true;
                max_residual = 0.0;
                break;
            };
            max_residual = update.residual;
            if update.residual <= tolerance {
                converged = true;
                break;
            }
            let damped = damp_message(
                match update.direction {
                    MessageDirection::VariableToFactor => {
                        &v_to_f[&(update.variable_id, update.factor_id)]
                    }
                    MessageDirection::FactorToVariable => {
                        &f_to_v[&(update.factor_id, update.variable_id)]
                    }
                },
                &update.candidate,
                damping,
            )?;
            match update.direction {
                MessageDirection::VariableToFactor => {
                    v_to_f.insert((update.variable_id, update.factor_id), damped);
                }
                MessageDirection::FactorToVariable => {
                    f_to_v.insert((update.factor_id, update.variable_id), damped);
                }
            }
            messages_updated += 1;
        }

        let result_variables = if query_variables.is_empty() {
            active.variables.clone()
        } else {
            query_variables.to_vec()
        };
        if persist {
            self.ensure_store_current()?;
        }
        let mut beliefs = BTreeMap::new();
        for variable_id in result_variables {
            if !active_variable_set.contains(&variable_id) {
                continue;
            }
            let belief =
                self.variable_belief(variable_id, &active_factor_set, &evidence, &f_to_v)?;
            beliefs.insert(variable_id, self.label_distribution(variable_id, &belief)?);
            if persist {
                self.posteriors.insert(variable_id, belief);
            }
        }

        if !converged {
            warnings.push(format!(
                "belief propagation did not converge within {max_iters} iterations"
            ));
        }
        if max_iters > 0 && iterations.saturating_mul(10) >= max_iters.saturating_mul(9) {
            warnings.push(format!(
                "belief propagation used {iterations} of {max_iters} configured iterations"
            ));
        }
        let diagnostics = belief_diagnostics(
            &active,
            iterations,
            max_iters,
            messages_updated,
            converged,
            max_residual,
            tolerance,
            damping,
        );

        let mut trace_id = None;
        if persist {
            if let Some(store) = &self.store {
                for (&variable_id, distribution) in &self.posteriors {
                    store.upsert_posterior(variable_id, distribution)?;
                }
            }
            trace_id = Some(self.add_trace(trace_payload(
                &active,
                iterations,
                messages_updated,
                converged,
                max_residual,
            ))?);
        }

        Ok(BeliefPropagationResult {
            beliefs,
            active,
            schedule: "residual_async".to_string(),
            iterations,
            messages_updated,
            converged,
            max_residual,
            trace_id,
            warnings,
            diagnostics,
        })
    }

    pub(crate) fn posterior(&self, variable_id: u64) -> Result<BTreeMap<String, f64>, String> {
        self.require_variable(variable_id)?;
        if let Some(posterior) = self.posteriors.get(&variable_id) {
            return self.label_distribution(variable_id, posterior);
        }
        let variable = self
            .variables
            .get(variable_id as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| format!("variable {variable_id} not found"))?;
        if !variable.posterior.is_empty() {
            let distribution =
                self.distribution_from_properties(variable_id, &variable.posterior, "posterior")?;
            return self.label_distribution(variable_id, &distribution);
        }
        let distribution = self.prior_distribution(variable_id)?;
        self.label_distribution(variable_id, &distribution)
    }

    pub(crate) fn local_propagate(
        &self,
        seeds: &HashMap<u64, f64>,
        radius: usize,
        query_nodes: Option<&[u64]>,
        edge_type: Option<&str>,
        edge_property: &str,
        damping: f64,
    ) -> Result<HashMap<u64, f64>, String> {
        if !(0.0..=1.0).contains(&damping) {
            return Err("damping must be between 0.0 and 1.0".to_string());
        }
        for (&node_id, &probability) in seeds {
            self.require_node(node_id)?;
            if !probability.is_finite() || probability < 0.0 {
                return Err("seed probabilities must be finite and non-negative".to_string());
            }
        }

        let mut seed_nodes = seeds.keys().copied().collect::<Vec<_>>();
        if let Some(query_nodes) = query_nodes {
            seed_nodes.extend_from_slice(query_nodes);
        }
        let mut truncated = false;
        let active_nodes = expand_nodes(self, &seed_nodes, radius, 10000, &mut truncated)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut current = seeds.clone();
        let mut accumulated = seeds.clone();
        for _ in 0..radius {
            let mut next = HashMap::new();
            for (&source, &probability) in &current {
                if !active_nodes.contains(&source) {
                    continue;
                }
                for edge_id in self.edge_ids_for_node(source, true) {
                    let Some(edge) = self.edge_record(edge_id) else {
                        continue;
                    };
                    if !active_nodes.contains(&edge.target) {
                        continue;
                    }
                    if let Some(filter) = edge_type {
                        if edge.edge_type != filter {
                            continue;
                        }
                    }
                    let weight = super::adjacency::edge_weight(edge, edge_property)?;
                    *next.entry(edge.target).or_insert(0.0) += probability * weight * damping;
                }
            }
            if next.is_empty() {
                break;
            }
            for (&node_id, &probability) in &next {
                *accumulated.entry(node_id).or_insert(0.0) += probability;
            }
            current = next;
        }
        Ok(accumulated)
    }

    fn best_message_update(
        &self,
        active_factors: &BTreeSet<u64>,
        evidence: &HashMap<u64, String>,
        v_to_f: &HashMap<(u64, u64), Vec<f64>>,
        f_to_v: &HashMap<(u64, u64), Vec<f64>>,
    ) -> Result<Option<MessageUpdate>, String> {
        let mut best = None::<MessageUpdate>;
        for &factor_id in active_factors {
            let table = self
                .factor_tables
                .get(&factor_id)
                .ok_or_else(|| format!("factor table {factor_id} not found"))?;
            for &variable_id in &table.variables {
                let variable_candidate =
                    self.variable_to_factor_message(variable_id, factor_id, evidence, f_to_v)?;
                let variable_old = &v_to_f[&(variable_id, factor_id)];
                let variable_residual = l1_residual(variable_old, &variable_candidate);
                set_best(
                    &mut best,
                    MessageUpdate {
                        factor_id,
                        variable_id,
                        direction: MessageDirection::VariableToFactor,
                        candidate: variable_candidate,
                        residual: variable_residual,
                    },
                );

                let factor_candidate =
                    self.factor_to_variable_message(factor_id, variable_id, v_to_f)?;
                let factor_old = &f_to_v[&(factor_id, variable_id)];
                let factor_residual = l1_residual(factor_old, &factor_candidate);
                set_best(
                    &mut best,
                    MessageUpdate {
                        factor_id,
                        variable_id,
                        direction: MessageDirection::FactorToVariable,
                        candidate: factor_candidate,
                        residual: factor_residual,
                    },
                );
            }
        }
        Ok(best)
    }

    fn variable_to_factor_message(
        &self,
        variable_id: u64,
        target_factor: u64,
        evidence: &HashMap<u64, String>,
        f_to_v: &HashMap<(u64, u64), Vec<f64>>,
    ) -> Result<Vec<f64>, String> {
        if let Some(state) = evidence.get(&variable_id) {
            return self.hard_evidence(variable_id, state);
        }
        let mut message = self.prior_distribution(variable_id)?;
        for (&(factor_id, message_variable), incoming) in f_to_v {
            if factor_id == target_factor || message_variable != variable_id {
                continue;
            }
            multiply_in_place(&mut message, incoming);
        }
        normalize(&message)
    }

    fn factor_to_variable_message(
        &self,
        factor_id: u64,
        target_variable: u64,
        v_to_f: &HashMap<(u64, u64), Vec<f64>>,
    ) -> Result<Vec<f64>, String> {
        let table = self
            .factor_tables
            .get(&factor_id)
            .ok_or_else(|| format!("factor table {factor_id} not found"))?;
        let target_position = table
            .variables
            .iter()
            .position(|variable_id| *variable_id == target_variable)
            .ok_or_else(|| format!("variable {target_variable} is not in factor {factor_id}"))?;
        let cardinalities = self.cardinalities(&table.variables)?;
        let strides = table_strides(&cardinalities, table.is_cpd);
        let mut result = vec![0.0; cardinalities[target_position]];

        for (flat_index, potential) in table.values.iter().enumerate() {
            if *potential == 0.0 {
                continue;
            }
            let mut product = *potential;
            let mut target_state = 0usize;
            for (position, &variable_id) in table.variables.iter().enumerate() {
                let state = (flat_index / strides[position]) % cardinalities[position];
                if position == target_position {
                    target_state = state;
                    continue;
                }
                product *= v_to_f[&(variable_id, factor_id)][state];
            }
            result[target_state] += product;
        }
        normalize(&result)
    }

    fn variable_belief(
        &self,
        variable_id: u64,
        active_factors: &BTreeSet<u64>,
        evidence: &HashMap<u64, String>,
        f_to_v: &HashMap<(u64, u64), Vec<f64>>,
    ) -> Result<Vec<f64>, String> {
        if let Some(state) = evidence.get(&variable_id) {
            return self.hard_evidence(variable_id, state);
        }
        let mut belief = self.prior_distribution(variable_id)?;
        for &factor_id in active_factors {
            let Some(table) = self.factor_tables.get(&factor_id) else {
                continue;
            };
            if !table.variables.contains(&variable_id) {
                continue;
            }
            if let Some(message) = f_to_v.get(&(factor_id, variable_id)) {
                multiply_in_place(&mut belief, message);
            }
        }
        normalize(&belief)
    }

    fn evidence_for_run(
        &self,
        runtime_evidence: &HashMap<u64, String>,
    ) -> Result<HashMap<u64, String>, String> {
        let mut evidence = HashMap::new();
        for record in self.evidence.iter().flatten() {
            if let Some(PropertyValue::String(state)) = record.payload.get("state") {
                self.state_index(record.variable_id, state)?;
                evidence.insert(record.variable_id, state.clone());
            }
        }
        for (&variable_id, state) in runtime_evidence {
            self.state_index(variable_id, state)?;
            evidence.insert(variable_id, state.clone());
        }
        Ok(evidence)
    }

    fn prior_distribution(&self, variable_id: u64) -> Result<Vec<f64>, String> {
        let variable = self
            .variables
            .get(variable_id as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| format!("variable {variable_id} not found"))?;
        self.distribution_from_properties(variable_id, &variable.prior, "prior")
    }

    fn distribution_from_properties(
        &self,
        variable_id: u64,
        properties: &PropertyMap,
        label: &str,
    ) -> Result<Vec<f64>, String> {
        let variable = self
            .variables
            .get(variable_id as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| format!("variable {variable_id} not found"))?;
        let mut distribution = Vec::with_capacity(variable.states.len());
        let mut has_state_prior = false;
        for state in &variable.states {
            if let Some(value) = properties.get(state) {
                let value = value
                    .as_f64()
                    .ok_or_else(|| format!("{label} for state {state:?} must be numeric"))?;
                if !value.is_finite() || value < 0.0 {
                    return Err(format!("{label} values must be finite and non-negative"));
                }
                distribution.push(value);
                has_state_prior = true;
            } else {
                distribution.push(0.0);
            }
        }
        if !has_state_prior
            && variable.states.len() == 2
            && variable.states[0] == "false"
            && variable.states[1] == "true"
        {
            if let Some(value) = properties.get("p") {
                let p = value
                    .as_f64()
                    .ok_or_else(|| format!("binary {label} key 'p' must be numeric"))?;
                if !(0.0..=1.0).contains(&p) {
                    return Err(format!(
                        "binary {label} key 'p' must be between 0.0 and 1.0"
                    ));
                }
                distribution = vec![1.0 - p, p];
                has_state_prior = true;
            }
        }
        if has_state_prior {
            normalize(&distribution)
        } else {
            Ok(uniform(variable.states.len()))
        }
    }

    fn hard_evidence(&self, variable_id: u64, state: &str) -> Result<Vec<f64>, String> {
        let index = self.state_index(variable_id, state)?;
        let mut distribution = vec![0.0; self.variable_states(variable_id)?.len()];
        distribution[index] = 1.0;
        Ok(distribution)
    }

    fn state_index(&self, variable_id: u64, state: &str) -> Result<usize, String> {
        self.variable_states(variable_id)?
            .iter()
            .position(|candidate| candidate == state)
            .ok_or_else(|| format!("state {state:?} not found for variable {variable_id}"))
    }

    fn label_distribution(
        &self,
        variable_id: u64,
        distribution: &[f64],
    ) -> Result<BTreeMap<String, f64>, String> {
        let states = self.variable_states(variable_id)?;
        if states.len() != distribution.len() {
            return Err(format!(
                "distribution for variable {variable_id} has invalid length"
            ));
        }
        Ok(states
            .iter()
            .cloned()
            .zip(distribution.iter().copied())
            .collect())
    }

    fn cardinalities(&self, variables: &[u64]) -> Result<Vec<usize>, String> {
        variables
            .iter()
            .map(|&variable_id| self.variable_states(variable_id).map(|states| states.len()))
            .collect()
    }

    fn variable_owner(&self, variable_id: u64) -> Option<u64> {
        self.variables
            .get(variable_id as usize)
            .and_then(Option::as_ref)
            .and_then(|variable| variable.owner_id)
    }

    fn existing_variable_ids(&self) -> Vec<u64> {
        self.variables
            .iter()
            .enumerate()
            .filter_map(|(id, variable)| variable.as_ref().map(|_| id as u64))
            .collect()
    }
}

fn expand_nodes(
    graph: &GraphCore,
    seeds: &[u64],
    radius: usize,
    max_nodes: usize,
    truncated: &mut bool,
) -> Result<Vec<u64>, String> {
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::new();
    for &node_id in seeds {
        graph.require_node(node_id)?;
        if visited.len() >= max_nodes {
            *truncated = true;
            break;
        }
        if visited.insert(node_id) {
            queue.push_back((node_id, 0usize));
        }
    }
    while let Some((node_id, depth)) = queue.pop_front() {
        if depth >= radius {
            continue;
        }
        for next in graph.neighbors_for_direction(node_id, Direction::Both, None) {
            if visited.contains(&next) {
                continue;
            }
            if visited.len() >= max_nodes {
                *truncated = true;
                continue;
            }
            visited.insert(next);
            queue.push_back((next, depth + 1));
        }
    }
    Ok(visited.into_iter().collect())
}

fn insert_capped_node(
    nodes: &mut BTreeSet<u64>,
    node_id: u64,
    max_nodes: usize,
    truncated: &mut bool,
) {
    if nodes.contains(&node_id) {
        return;
    }
    if nodes.len() < max_nodes {
        nodes.insert(node_id);
    } else {
        *truncated = true;
    }
}

fn uniform(len: usize) -> Vec<f64> {
    vec![1.0 / len as f64; len]
}

fn strides(cardinalities: &[usize]) -> Vec<usize> {
    let mut strides = vec![1; cardinalities.len()];
    let mut product = 1usize;
    for index in (0..cardinalities.len()).rev() {
        strides[index] = product;
        product *= cardinalities[index];
    }
    strides
}

fn cpd_strides(cardinalities: &[usize]) -> Vec<usize> {
    let mut strides = vec![1; cardinalities.len()];
    if cardinalities.is_empty() {
        return strides;
    }
    strides[0] = 1;
    let mut product = cardinalities[0];
    for index in (1..cardinalities.len()).rev() {
        strides[index] = product;
        product *= cardinalities[index];
    }
    strides
}

fn table_strides(cardinalities: &[usize], is_cpd: bool) -> Vec<usize> {
    if is_cpd {
        cpd_strides(cardinalities)
    } else {
        strides(cardinalities)
    }
}

fn multiply_in_place(values: &mut [f64], factors: &[f64]) {
    for (value, factor) in values.iter_mut().zip(factors) {
        *value *= factor;
    }
}

fn l1_residual(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| (left - right).abs())
        .sum()
}

fn damp_message(old: &[f64], candidate: &[f64], damping: f64) -> Result<Vec<f64>, String> {
    let values = old
        .iter()
        .zip(candidate)
        .map(|(old, candidate)| (1.0 - damping) * candidate + damping * old)
        .collect::<Vec<_>>();
    normalize(&values)
}

fn set_best(best: &mut Option<MessageUpdate>, candidate: MessageUpdate) {
    if best
        .as_ref()
        .is_none_or(|current| candidate.residual > current.residual)
    {
        *best = Some(candidate);
    }
}

fn trace_payload(
    active: &ActiveSubgraph,
    iterations: usize,
    messages_updated: usize,
    converged: bool,
    max_residual: f64,
) -> PropertyMap {
    HashMap::from([
        (
            "kind".to_string(),
            PropertyValue::String("belief_propagation".to_string()),
        ),
        (
            "schedule".to_string(),
            PropertyValue::String("residual_async".to_string()),
        ),
        (
            "iterations".to_string(),
            PropertyValue::Int(iterations as i64),
        ),
        (
            "messages_updated".to_string(),
            PropertyValue::Int(messages_updated as i64),
        ),
        ("converged".to_string(), PropertyValue::Bool(converged)),
        (
            "max_residual".to_string(),
            PropertyValue::Float(max_residual),
        ),
        (
            "active_variables".to_string(),
            PropertyValue::Int(active.variables.len() as i64),
        ),
        (
            "active_factors".to_string(),
            PropertyValue::Int(active.factors.len() as i64),
        ),
        (
            "active_graph_nodes".to_string(),
            PropertyValue::Int(active.graph_nodes.len() as i64),
        ),
        (
            "truncated".to_string(),
            PropertyValue::Bool(active.truncated),
        ),
    ])
}

fn active_subgraph_warnings(active: &ActiveSubgraph) -> Vec<String> {
    let mut warnings = Vec::new();
    if active.truncated {
        warnings
            .push("active subgraph was truncated by max_nodes or max_factors limits".to_string());
    }
    if active.variables.len() > 1000
        || active.factors.len() > 5000
        || active.graph_nodes.len() > 5000
    {
        warnings.push(format!(
            "large active subgraph: {} variables, {} factors, {} graph nodes",
            active.variables.len(),
            active.factors.len(),
            active.graph_nodes.len()
        ));
    }
    warnings
}

fn belief_diagnostics(
    active: &ActiveSubgraph,
    iterations: usize,
    max_iters: usize,
    messages_updated: usize,
    converged: bool,
    max_residual: f64,
    tolerance: f64,
    damping: f64,
) -> BTreeMap<String, PropertyValue> {
    BTreeMap::from([
        (
            "active_variables".to_string(),
            PropertyValue::Int(active.variables.len() as i64),
        ),
        (
            "active_factors".to_string(),
            PropertyValue::Int(active.factors.len() as i64),
        ),
        (
            "active_graph_nodes".to_string(),
            PropertyValue::Int(active.graph_nodes.len() as i64),
        ),
        (
            "boundary_variables".to_string(),
            PropertyValue::Int(active.boundary_variables.len() as i64),
        ),
        (
            "truncated".to_string(),
            PropertyValue::Bool(active.truncated),
        ),
        (
            "iterations".to_string(),
            PropertyValue::Int(iterations as i64),
        ),
        (
            "max_iters".to_string(),
            PropertyValue::Int(max_iters as i64),
        ),
        (
            "messages_updated".to_string(),
            PropertyValue::Int(messages_updated as i64),
        ),
        ("converged".to_string(), PropertyValue::Bool(converged)),
        (
            "max_residual".to_string(),
            PropertyValue::Float(max_residual),
        ),
        ("tolerance".to_string(), PropertyValue::Float(tolerance)),
        ("damping".to_string(), PropertyValue::Float(damping)),
    ])
}
