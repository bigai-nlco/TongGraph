use super::properties::{validate_non_empty, validate_properties};
use super::GraphCore;
use crate::models::{EvidenceRecord, FactorRecord, PropertyMap, TraceRecord, VariableRecord};

impl GraphCore {
    pub(crate) fn add_variable(
        &mut self,
        owner_id: Option<u64>,
        domain: String,
        prior: PropertyMap,
        posterior: PropertyMap,
    ) -> Result<u64, String> {
        validate_non_empty("domain", &domain)?;
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
