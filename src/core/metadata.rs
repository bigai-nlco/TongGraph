use super::properties::{validate_non_empty, validate_properties};
use super::GraphCore;
use crate::models::{
    EvidenceRecord, FactorRecord, FactorTableRecord, PropertyMap, TraceRecord, VariableRecord,
};

impl GraphCore {
    pub(crate) fn add_variable(
        &mut self,
        owner_id: Option<u64>,
        domain: String,
        states: Option<Vec<String>>,
        prior: PropertyMap,
        posterior: PropertyMap,
    ) -> Result<u64, String> {
        validate_non_empty("domain", &domain)?;
        let states = states_for_domain(&domain, states)?;
        validate_properties(&prior)?;
        validate_properties(&posterior)?;
        if let Some(owner_id) = owner_id {
            self.require_node(owner_id)?;
        }

        let id = self.next_variable_id;
        let record = VariableRecord {
            id,
            owner_id,
            domain,
            states,
            prior,
            posterior,
        };

        if let Some(store) = &self.store {
            store.insert_variable(&record)?;
        }
        self.insert_variable_record(record)?;
        Ok(id)
    }

    pub(crate) fn add_factor(
        &mut self,
        input_variables: Vec<u64>,
        output_variables: Vec<u64>,
        function: String,
        parameters: PropertyMap,
    ) -> Result<u64, String> {
        validate_non_empty("function", &function)?;
        validate_properties(&parameters)?;
        for &variable_id in input_variables.iter().chain(output_variables.iter()) {
            self.require_variable(variable_id)?;
        }

        let id = self.next_factor_id;
        let record = FactorRecord {
            id,
            input_variables,
            output_variables,
            function,
            parameters,
        };

        if let Some(store) = &self.store {
            store.insert_factor(&record)?;
        }
        self.insert_factor_record(record)?;
        Ok(id)
    }

    pub(crate) fn add_factor_table(
        &mut self,
        variables: Vec<u64>,
        values: Vec<f64>,
    ) -> Result<u64, String> {
        validate_factor_table(self, &variables, &values, false)?;
        let factor_id = self.add_factor(
            variables.clone(),
            Vec::new(),
            "factor_table".to_string(),
            PropertyMap::new(),
        )?;
        let record = FactorTableRecord {
            factor_id,
            variables,
            values,
            is_cpd: false,
        };
        if let Some(store) = &self.store {
            store.insert_factor_table(&record)?;
        }
        self.insert_factor_table_record(record)?;
        Ok(factor_id)
    }

    pub(crate) fn add_cpd(
        &mut self,
        variable_id: u64,
        parent_variables: Vec<u64>,
        values: Vec<f64>,
    ) -> Result<u64, String> {
        self.require_variable(variable_id)?;
        let mut variables = Vec::with_capacity(parent_variables.len() + 1);
        variables.push(variable_id);
        variables.extend(parent_variables);
        validate_factor_table(self, &variables, &values, true)?;
        let factor_id = self.add_factor(
            variables.iter().copied().skip(1).collect(),
            vec![variable_id],
            "cpd".to_string(),
            PropertyMap::new(),
        )?;
        let record = FactorTableRecord {
            factor_id,
            variables,
            values,
            is_cpd: true,
        };
        if let Some(store) = &self.store {
            store.insert_factor_table(&record)?;
        }
        self.insert_factor_table_record(record)?;
        Ok(factor_id)
    }

    pub(crate) fn add_evidence(
        &mut self,
        variable_id: u64,
        payload: PropertyMap,
    ) -> Result<u64, String> {
        self.require_variable(variable_id)?;
        validate_properties(&payload)?;

        let id = self.next_evidence_id;
        let record = EvidenceRecord {
            id,
            variable_id,
            payload,
        };

        if let Some(store) = &self.store {
            store.insert_evidence(&record)?;
        }
        self.insert_evidence_record(record)?;
        Ok(id)
    }

    pub(crate) fn add_trace(&mut self, payload: PropertyMap) -> Result<u64, String> {
        validate_properties(&payload)?;

        let id = self.next_trace_id;
        let record = TraceRecord { id, payload };

        if let Some(store) = &self.store {
            store.insert_trace(&record)?;
        }
        self.insert_trace_record(record)?;
        Ok(id)
    }

    pub(super) fn insert_loaded_variable(&mut self, record: VariableRecord) -> Result<(), String> {
        if let Some(owner_id) = record.owner_id {
            self.require_node(owner_id)?;
        }
        self.insert_variable_record(record)
    }

    pub(super) fn insert_loaded_factor(&mut self, record: FactorRecord) -> Result<(), String> {
        for &variable_id in record
            .input_variables
            .iter()
            .chain(record.output_variables.iter())
        {
            self.require_variable(variable_id)?;
        }
        self.insert_factor_record(record)
    }

    pub(super) fn insert_loaded_factor_table(
        &mut self,
        record: FactorTableRecord,
    ) -> Result<(), String> {
        validate_factor_table(self, &record.variables, &record.values, record.is_cpd)?;
        self.require_factor(record.factor_id)?;
        self.insert_factor_table_record(record)
    }

    pub(super) fn insert_loaded_posterior(
        &mut self,
        variable_id: u64,
        posterior: Vec<f64>,
    ) -> Result<(), String> {
        self.require_variable(variable_id)?;
        if posterior.len() != self.variable_states(variable_id)?.len() {
            return Err(format!(
                "posterior for variable {variable_id} has invalid length"
            ));
        }
        let _ = normalize(&posterior)?;
        self.posteriors.insert(variable_id, posterior);
        Ok(())
    }

    pub(super) fn insert_loaded_evidence(&mut self, record: EvidenceRecord) -> Result<(), String> {
        self.require_variable(record.variable_id)?;
        self.insert_evidence_record(record)
    }

    pub(super) fn insert_loaded_trace(&mut self, record: TraceRecord) -> Result<(), String> {
        self.insert_trace_record(record)
    }

    fn insert_variable_record(&mut self, record: VariableRecord) -> Result<(), String> {
        let id = record.id;
        self.ensure_variable_slot(id);
        if self.variables[id as usize].is_some() {
            return Err(format!("variable id {id} already exists"));
        }
        self.variables[id as usize] = Some(record);
        self.next_variable_id = self.next_variable_id.max(id + 1);
        Ok(())
    }

    fn insert_factor_record(&mut self, record: FactorRecord) -> Result<(), String> {
        let id = record.id;
        self.ensure_factor_slot(id);
        if self.factors[id as usize].is_some() {
            return Err(format!("factor id {id} already exists"));
        }
        self.factors[id as usize] = Some(record);
        self.next_factor_id = self.next_factor_id.max(id + 1);
        Ok(())
    }

    fn insert_factor_table_record(&mut self, record: FactorTableRecord) -> Result<(), String> {
        if self.factor_tables.contains_key(&record.factor_id) {
            return Err(format!(
                "factor table for factor {} already exists",
                record.factor_id
            ));
        }
        self.factor_tables.insert(record.factor_id, record);
        Ok(())
    }

    fn insert_evidence_record(&mut self, record: EvidenceRecord) -> Result<(), String> {
        let id = record.id;
        self.ensure_evidence_slot(id);
        if self.evidence[id as usize].is_some() {
            return Err(format!("evidence id {id} already exists"));
        }
        self.evidence[id as usize] = Some(record);
        self.next_evidence_id = self.next_evidence_id.max(id + 1);
        Ok(())
    }

    fn insert_trace_record(&mut self, record: TraceRecord) -> Result<(), String> {
        let id = record.id;
        self.ensure_trace_slot(id);
        if self.traces[id as usize].is_some() {
            return Err(format!("trace id {id} already exists"));
        }
        self.traces[id as usize] = Some(record);
        self.next_trace_id = self.next_trace_id.max(id + 1);
        Ok(())
    }

    pub(super) fn require_variable(&self, variable_id: u64) -> Result<(), String> {
        match self.variables.get(variable_id as usize) {
            Some(Some(_)) => Ok(()),
            _ => Err(format!("variable {variable_id} not found")),
        }
    }

    pub(super) fn require_factor(&self, factor_id: u64) -> Result<(), String> {
        match self.factors.get(factor_id as usize) {
            Some(Some(_)) => Ok(()),
            _ => Err(format!("factor {factor_id} not found")),
        }
    }

    pub(super) fn variable_states(&self, variable_id: u64) -> Result<&[String], String> {
        self.variables
            .get(variable_id as usize)
            .and_then(Option::as_ref)
            .map(|variable| variable.states.as_slice())
            .ok_or_else(|| format!("variable {variable_id} not found"))
    }

    fn ensure_variable_slot(&mut self, variable_id: u64) {
        let size = variable_id as usize + 1;
        if self.variables.len() < size {
            self.variables.resize_with(size, || None);
        }
    }

    fn ensure_factor_slot(&mut self, factor_id: u64) {
        let size = factor_id as usize + 1;
        if self.factors.len() < size {
            self.factors.resize_with(size, || None);
        }
    }

    fn ensure_evidence_slot(&mut self, evidence_id: u64) {
        let size = evidence_id as usize + 1;
        if self.evidence.len() < size {
            self.evidence.resize_with(size, || None);
        }
    }

    fn ensure_trace_slot(&mut self, trace_id: u64) {
        let size = trace_id as usize + 1;
        if self.traces.len() < size {
            self.traces.resize_with(size, || None);
        }
    }
}

fn states_for_domain(domain: &str, states: Option<Vec<String>>) -> Result<Vec<String>, String> {
    let states = match states {
        Some(states) => states,
        None if domain == "binary" => vec!["false".to_string(), "true".to_string()],
        None => {
            return Err(format!(
                "states are required for non-binary variable domain {domain:?}"
            ));
        }
    };
    if states.is_empty() {
        return Err("variable states cannot be empty".to_string());
    }
    let mut seen = std::collections::BTreeSet::new();
    for state in &states {
        validate_non_empty("state", state)?;
        if !seen.insert(state) {
            return Err(format!("duplicate state {state:?}"));
        }
    }
    Ok(states)
}

fn validate_factor_table(
    graph: &GraphCore,
    variables: &[u64],
    values: &[f64],
    is_cpd: bool,
) -> Result<(), String> {
    if variables.is_empty() {
        return Err("factor table variables cannot be empty".to_string());
    }
    let mut expected = 1usize;
    let mut seen_variables = std::collections::BTreeSet::new();
    for &variable_id in variables {
        graph.require_variable(variable_id)?;
        if !seen_variables.insert(variable_id) {
            return Err(format!("duplicate variable {variable_id} in factor table"));
        }
        expected = expected
            .checked_mul(graph.variable_states(variable_id)?.len())
            .ok_or_else(|| "factor table cardinality overflows".to_string())?;
    }
    if values.len() != expected {
        return Err(format!(
            "factor table has {} values but expected {expected}",
            values.len()
        ));
    }
    for value in values {
        if !value.is_finite() || *value < 0.0 {
            return Err("factor table values must be finite and non-negative".to_string());
        }
    }
    if values.iter().all(|value| *value == 0.0) {
        return Err("factor table cannot be all zero".to_string());
    }
    if is_cpd {
        let child_cardinality = graph.variable_states(variables[0])?.len();
        for chunk in values.chunks(child_cardinality) {
            if chunk.iter().all(|value| *value == 0.0) {
                return Err("CPD child distribution cannot be all zero".to_string());
            }
            let sum = chunk.iter().sum::<f64>();
            if (sum - 1.0).abs() > 1e-9 {
                return Err("CPD child distributions must sum to 1.0".to_string());
            }
        }
    }
    Ok(())
}

pub(super) fn normalize(values: &[f64]) -> Result<Vec<f64>, String> {
    let sum = values.iter().sum::<f64>();
    if !sum.is_finite() || sum <= 0.0 {
        return Err("distribution must have positive finite mass".to_string());
    }
    Ok(values.iter().map(|value| value / sum).collect())
}
