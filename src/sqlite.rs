use crate::codec::{
    decode_f64_list, decode_list, decode_map, decode_u64_list, encode_f64_list, encode_list,
    encode_map, encode_u64_list,
};
use crate::core::segment::ComputeSegment;
use crate::models::{
    EdgeRecord, EvidenceRecord, FactorRecord, FactorTableRecord, FullTextIndexDefinition,
    GraphChanges, NodeRecord, PropertyMap, TraceRecord, VariableRecord,
};
use std::ffi::{c_char, c_int, c_uchar, c_void, CStr, CString};
use std::fs;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::ptr;

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;
const SQLITE_NULL: c_int = 5;

type Sqlite3 = c_void;
type Sqlite3Stmt = c_void;

#[link(name = "sqlite3")]
extern "C" {
    fn sqlite3_open(filename: *const c_char, pp_db: *mut *mut Sqlite3) -> c_int;
    fn sqlite3_close(db: *mut Sqlite3) -> c_int;
    fn sqlite3_errmsg(db: *mut Sqlite3) -> *const c_char;
    fn sqlite3_exec(
        db: *mut Sqlite3,
        sql: *const c_char,
        callback: Option<
            unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_char, *mut *mut c_char) -> c_int,
        >,
        arg: *mut c_void,
        errmsg: *mut *mut c_char,
    ) -> c_int;
    fn sqlite3_free(ptr: *mut c_void);
    fn sqlite3_prepare_v2(
        db: *mut Sqlite3,
        sql: *const c_char,
        n_byte: c_int,
        pp_stmt: *mut *mut Sqlite3Stmt,
        pz_tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_finalize(stmt: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_reset(stmt: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_step(stmt: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_bind_int64(stmt: *mut Sqlite3Stmt, index: c_int, value: i64) -> c_int;
    fn sqlite3_bind_null(stmt: *mut Sqlite3Stmt, index: c_int) -> c_int;
    fn sqlite3_bind_text(
        stmt: *mut Sqlite3Stmt,
        index: c_int,
        value: *const c_char,
        n: c_int,
        destructor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;
    fn sqlite3_column_int64(stmt: *mut Sqlite3Stmt, index: c_int) -> i64;
    fn sqlite3_column_text(stmt: *mut Sqlite3Stmt, index: c_int) -> *const c_uchar;
    fn sqlite3_column_type(stmt: *mut Sqlite3Stmt, index: c_int) -> c_int;
}

pub(crate) trait GraphStore {
    fn insert_nodes(&self, nodes: &[NodeRecord]) -> Result<(), String>;
    fn insert_edges(&self, edges: &[EdgeRecord]) -> Result<(), String>;
    fn apply_graph_changes(&self, changes: &GraphChanges) -> Result<(), String>;
    fn insert_variable(&self, variable: &VariableRecord) -> Result<(), String>;
    fn insert_factor(&self, factor: &FactorRecord) -> Result<(), String>;
    fn insert_factor_table(&self, factor_table: &FactorTableRecord) -> Result<(), String>;
    fn insert_evidence(&self, evidence: &EvidenceRecord) -> Result<(), String>;
    fn insert_trace(&self, trace: &TraceRecord) -> Result<(), String>;
    fn load_nodes(&self) -> Result<Vec<NodeRecord>, String>;
    fn load_edges(&self) -> Result<Vec<EdgeRecord>, String>;
    fn load_variables(&self) -> Result<Vec<VariableRecord>, String>;
    fn load_factors(&self) -> Result<Vec<FactorRecord>, String>;
    fn load_factor_tables(&self) -> Result<Vec<FactorTableRecord>, String>;
    fn load_posteriors(&self) -> Result<Vec<(u64, Vec<f64>)>, String>;
    fn upsert_posterior(&self, variable_id: u64, values: &[f64]) -> Result<(), String>;
    fn load_evidence(&self) -> Result<Vec<EvidenceRecord>, String>;
    fn load_traces(&self) -> Result<Vec<TraceRecord>, String>;
    fn load_segment(
        &self,
        expected_nodes: usize,
        expected_edges: usize,
    ) -> Result<Option<ComputeSegment>, String>;
    fn save_segment(
        &self,
        segment: &ComputeSegment,
        node_count: usize,
        edge_count: usize,
    ) -> Result<(), String>;
    fn current_op_seq(&self) -> Result<u64, String>;
    fn load_next_ids(&self) -> Result<(Option<u64>, Option<u64>), String>;
    fn load_fulltext_indexes(&self) -> Result<Vec<FullTextIndexDefinition>, String>;
    fn create_fulltext_index(
        &self,
        definition: &FullTextIndexDefinition,
        nodes: &[NodeRecord],
        edges: &[EdgeRecord],
    ) -> Result<(), String>;
    fn drop_fulltext_index(&self, name: &str) -> Result<(), String>;
    fn rebuild_fulltext_indexes(
        &self,
        definitions: &[FullTextIndexDefinition],
        nodes: &[NodeRecord],
        edges: &[EdgeRecord],
    ) -> Result<(), String>;
    fn fulltext_candidates(
        &self,
        definition: &FullTextIndexDefinition,
        expression: &str,
    ) -> Result<Vec<u64>, String>;
    fn path(&self) -> String;
}

pub(crate) struct SqliteStore {
    db: *mut Sqlite3,
    path: PathBuf,
}

impl SqliteStore {
    pub(crate) fn open(path: &str) -> Result<Self, String> {
        let db_path = PathBuf::from(path);
        let path = CString::new(path).map_err(|_| "SQLite path contains NUL byte".to_string())?;
        let mut db = ptr::null_mut();
        let rc = unsafe { sqlite3_open(path.as_ptr(), &mut db) };
        if rc != SQLITE_OK {
            let message = if db.is_null() {
                "failed to open SQLite database".to_string()
            } else {
                sqlite_error(db)
            };
            if !db.is_null() {
                unsafe {
                    sqlite3_close(db);
                }
            }
            return Err(message);
        }

        let store = Self { db, path: db_path };
        store.initialize()?;
        Ok(store)
    }

    pub(crate) fn insert_nodes(&self, nodes: &[NodeRecord]) -> Result<(), String> {
        self.insert_graph_records(nodes, &[])
    }

    pub(crate) fn insert_edges(&self, edges: &[EdgeRecord]) -> Result<(), String> {
        self.insert_graph_records(&[], edges)
    }

    pub(crate) fn insert_graph_records(
        &self,
        nodes: &[NodeRecord],
        edges: &[EdgeRecord],
    ) -> Result<(), String> {
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
            for node in nodes {
                self.insert_node_row(node)?;
            }
            for edge in edges {
                self.insert_edge_row(edge)?;
            }
            Ok(())
        })();
        self.finish_transaction(result)
    }

    pub(crate) fn apply_graph_changes(&self, changes: &GraphChanges) -> Result<(), String> {
        if changes.is_empty() {
            return Ok(());
        }
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
            for edge_id in &changes.delete_edge_ids {
                self.delete_edge_row(*edge_id)?;
            }
            for node_id in &changes.delete_node_ids {
                self.delete_node_row(*node_id)?;
            }
            for node in &changes.upsert_nodes {
                self.upsert_node_row(node)?;
            }
            for edge in &changes.upsert_edges {
                self.upsert_edge_row(edge)?;
            }
            self.reset_property_catalog()?;
            self.upsert_metadata("next_node_id", &changes.next_node_id.to_string())?;
            self.upsert_metadata("next_edge_id", &changes.next_edge_id.to_string())?;
            if changes.counters_changed
                && changes.upsert_nodes.is_empty()
                && changes.upsert_edges.is_empty()
                && changes.delete_node_ids.is_empty()
                && changes.delete_edge_ids.is_empty()
            {
                self.append_op("advance_graph_ids", 0, "")?;
            }
            Ok(())
        })();
        self.finish_transaction(result)
    }

    pub(crate) fn load_fulltext_indexes(&self) -> Result<Vec<FullTextIndexDefinition>, String> {
        let mut stmt = self.prepare(
            "SELECT name, target, properties, tokenizer FROM fulltext_indexes ORDER BY name;",
        )?;
        let mut definitions = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => definitions.push(FullTextIndexDefinition {
                    name: stmt.column_text(0)?,
                    target: stmt.column_text(1)?,
                    properties: decode_list(&stmt.column_text(2)?),
                    tokenizer: stmt.column_text(3)?,
                }),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        Ok(definitions)
    }

    pub(crate) fn create_fulltext_index(
        &self,
        definition: &FullTextIndexDefinition,
        nodes: &[NodeRecord],
        edges: &[EdgeRecord],
    ) -> Result<(), String> {
        self.ensure_fulltext_table(&definition.tokenizer)?;
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
            let mut stmt = self.prepare(
                "INSERT INTO fulltext_indexes (name, target, properties, tokenizer) VALUES (?1, ?2, ?3, ?4);",
            )?;
            stmt.bind_text(1, &definition.name)?;
            stmt.bind_text(2, &definition.target)?;
            stmt.bind_text(3, &encode_list(&definition.properties))?;
            stmt.bind_text(4, &definition.tokenizer)?;
            stmt.step_done()?;
            self.rebuild_fulltext_definition(definition, nodes, edges)?;
            self.append_op("create_fulltext_index", 0, &definition.name)?;
            Ok(())
        })();
        self.finish_transaction(result)
    }

    pub(crate) fn drop_fulltext_index(&self, name: &str) -> Result<(), String> {
        let definition = self
            .load_fulltext_indexes()?
            .into_iter()
            .find(|definition| definition.name == name)
            .ok_or_else(|| format!("full-text index {name:?} not found"))?;
        self.ensure_fulltext_table(&definition.tokenizer)?;
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
            self.delete_fulltext_index_rows(&definition)?;
            let mut stmt = self.prepare("DELETE FROM fulltext_indexes WHERE name = ?1;")?;
            stmt.bind_text(1, name)?;
            stmt.step_done()?;
            self.append_op("drop_fulltext_index", 0, name)?;
            Ok(())
        })();
        self.finish_transaction(result)
    }

    pub(crate) fn rebuild_fulltext_indexes(
        &self,
        definitions: &[FullTextIndexDefinition],
        nodes: &[NodeRecord],
        edges: &[EdgeRecord],
    ) -> Result<(), String> {
        if definitions.is_empty() {
            return Ok(());
        }
        let mut tokenizers = definitions
            .iter()
            .map(|definition| definition.tokenizer.as_str())
            .collect::<Vec<_>>();
        tokenizers.sort_unstable();
        tokenizers.dedup();
        let mut rebuild = definitions.to_vec();
        for tokenizer in tokenizers {
            if self.ensure_healthy_fulltext_table(tokenizer)? {
                for definition in self
                    .load_fulltext_indexes()?
                    .into_iter()
                    .filter(|definition| definition.tokenizer == tokenizer)
                {
                    if !rebuild
                        .iter()
                        .any(|existing| existing.name == definition.name)
                    {
                        rebuild.push(definition);
                    }
                }
            }
        }
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
            for definition in &rebuild {
                self.rebuild_fulltext_definition(definition, nodes, edges)?;
            }
            Ok(())
        })();
        self.finish_transaction(result)
    }

    pub(crate) fn fulltext_candidates(
        &self,
        definition: &FullTextIndexDefinition,
        expression: &str,
    ) -> Result<Vec<u64>, String> {
        self.ensure_fulltext_table(&definition.tokenizer)?;
        let table = fulltext_table(&definition.tokenizer)?;
        let sql = format!(
            "SELECT entity_id FROM {table} WHERE {table} MATCH ?1 AND index_name = ?2 ORDER BY entity_id;"
        );
        let mut stmt = self.prepare(&sql)?;
        stmt.bind_text(1, expression)?;
        stmt.bind_text(2, &definition.name)?;
        let mut ids = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => ids.push(stmt.column_i64(0)?),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        Ok(ids)
    }

    fn ensure_healthy_fulltext_table(&self, tokenizer: &str) -> Result<bool, String> {
        self.ensure_fulltext_table(tokenizer)?;
        let table = fulltext_table(tokenizer)?;
        let integrity_check = format!("INSERT INTO {table} ({table}) VALUES ('integrity-check');");
        if self.exec(&integrity_check).is_ok() {
            return Ok(false);
        }
        self.exec(&format!("DROP TABLE IF EXISTS {table};"))
            .map_err(|error| {
                format!("failed to remove corrupt SQLite FTS5 table {table:?}: {error}")
            })?;
        self.ensure_fulltext_table(tokenizer)?;
        Ok(true)
    }

    fn ensure_fulltext_table(&self, tokenizer: &str) -> Result<(), String> {
        let (table, tokenizer_sql) = match tokenizer {
            "unicode61" => ("fulltext_unicode", "unicode61"),
            "trigram" => ("fulltext_trigram", "trigram"),
            _ => return Err(format!("unknown full-text tokenizer {tokenizer:?}")),
        };
        self.exec(&format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {table} USING fts5(index_name UNINDEXED, target UNINDEXED, entity_id UNINDEXED, content, tokenize='{tokenizer_sql}');"
        ))
        .map_err(|error| format!("SQLite FTS5 tokenizer {tokenizer:?} is unavailable: {error}"))
    }

    fn rebuild_fulltext_definition(
        &self,
        definition: &FullTextIndexDefinition,
        nodes: &[NodeRecord],
        edges: &[EdgeRecord],
    ) -> Result<(), String> {
        self.delete_fulltext_index_rows(definition)?;
        match definition.target.as_str() {
            "node" => {
                for node in nodes {
                    self.insert_fulltext_entity(definition, node.id, &node.properties)?;
                }
            }
            "edge" => {
                for edge in edges {
                    self.insert_fulltext_entity(definition, edge.id, &edge.properties)?;
                }
            }
            _ => return Err(format!("unknown full-text target {:?}", definition.target)),
        }
        Ok(())
    }

    fn delete_fulltext_index_rows(
        &self,
        definition: &FullTextIndexDefinition,
    ) -> Result<(), String> {
        let table = fulltext_table(&definition.tokenizer)?;
        let mut stmt = self.prepare(&format!("DELETE FROM {table} WHERE index_name = ?1;"))?;
        stmt.bind_text(1, &definition.name)?;
        stmt.step_done()
    }

    fn sync_fulltext_entity(
        &self,
        target: &str,
        entity_id: u64,
        properties: Option<&PropertyMap>,
    ) -> Result<(), String> {
        for definition in self
            .load_fulltext_indexes()?
            .into_iter()
            .filter(|definition| definition.target == target)
        {
            self.ensure_fulltext_table(&definition.tokenizer)?;
            let table = fulltext_table(&definition.tokenizer)?;
            let mut delete = self.prepare(&format!(
                "DELETE FROM {table} WHERE index_name = ?1 AND entity_id = ?2;"
            ))?;
            delete.bind_text(1, &definition.name)?;
            delete.bind_i64(2, entity_id)?;
            delete.step_done()?;
            if let Some(properties) = properties {
                self.insert_fulltext_entity(&definition, entity_id, properties)?;
            }
        }
        Ok(())
    }

    fn insert_fulltext_entity(
        &self,
        definition: &FullTextIndexDefinition,
        entity_id: u64,
        properties: &PropertyMap,
    ) -> Result<(), String> {
        let content = definition
            .properties
            .iter()
            .filter_map(|property| match properties.get(property) {
                Some(crate::models::PropertyValue::String(value)) if !value.trim().is_empty() => {
                    Some(value.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        if content.is_empty() {
            return Ok(());
        }
        let table = fulltext_table(&definition.tokenizer)?;
        let mut stmt = self.prepare(&format!(
            "INSERT INTO {table} (index_name, target, entity_id, content) VALUES (?1, ?2, ?3, ?4);"
        ))?;
        stmt.bind_text(1, &definition.name)?;
        stmt.bind_text(2, &definition.target)?;
        stmt.bind_i64(3, entity_id)?;
        stmt.bind_text(4, &content)?;
        stmt.step_done()
    }

    pub(crate) fn insert_variable(&self, variable: &VariableRecord) -> Result<(), String> {
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
            let mut stmt = self.prepare(
                "INSERT INTO variables (id, owner_id, domain, prior, posterior) VALUES (?1, ?2, ?3, ?4, ?5);",
            )?;
            stmt.bind_i64(1, variable.id)?;
            stmt.bind_optional_i64(2, variable.owner_id)?;
            stmt.bind_text(3, &variable.domain)?;
            stmt.bind_text(4, &encode_map(&variable.prior))?;
            stmt.bind_text(5, &encode_map(&variable.posterior))?;
            stmt.step_done()?;
            self.insert_variable_state_rows(variable.id, &variable.states)?;
            self.append_op("add_variable", variable.id, &variable.domain)?;
            Ok(())
        })();
        self.finish_transaction(result)
    }

    pub(crate) fn insert_factor(&self, factor: &FactorRecord) -> Result<(), String> {
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
            let mut stmt = self.prepare(
                "INSERT INTO factors (id, input_variables, output_variables, function, parameters) VALUES (?1, ?2, ?3, ?4, ?5);",
            )?;
            stmt.bind_i64(1, factor.id)?;
            stmt.bind_text(2, &encode_u64_list(&factor.input_variables))?;
            stmt.bind_text(3, &encode_u64_list(&factor.output_variables))?;
            stmt.bind_text(4, &factor.function)?;
            stmt.bind_text(5, &encode_map(&factor.parameters))?;
            stmt.step_done()?;
            self.append_op("add_factor", factor.id, &factor.function)?;
            Ok(())
        })();
        self.finish_transaction(result)
    }

    pub(crate) fn insert_factor_table(
        &self,
        factor_table: &FactorTableRecord,
    ) -> Result<(), String> {
        let mut stmt = self.prepare(
            "INSERT OR REPLACE INTO factor_tables (factor_id, variables, values_text, is_cpd)
             VALUES (?1, ?2, ?3, ?4);",
        )?;
        stmt.bind_i64(1, factor_table.factor_id)?;
        stmt.bind_text(2, &encode_u64_list(&factor_table.variables))?;
        stmt.bind_text(3, &encode_f64_list(&factor_table.values))?;
        stmt.bind_i64(4, if factor_table.is_cpd { 1 } else { 0 })?;
        stmt.step_done()
    }

    pub(crate) fn insert_evidence(&self, evidence: &EvidenceRecord) -> Result<(), String> {
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
            let mut stmt = self
                .prepare("INSERT INTO evidence (id, variable_id, payload) VALUES (?1, ?2, ?3);")?;
            stmt.bind_i64(1, evidence.id)?;
            stmt.bind_i64(2, evidence.variable_id)?;
            stmt.bind_text(3, &encode_map(&evidence.payload))?;
            stmt.step_done()?;
            self.append_op(
                "add_evidence",
                evidence.id,
                &evidence.variable_id.to_string(),
            )?;
            Ok(())
        })();
        self.finish_transaction(result)
    }

    pub(crate) fn insert_trace(&self, trace: &TraceRecord) -> Result<(), String> {
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
            let mut stmt = self.prepare("INSERT INTO traces (id, payload) VALUES (?1, ?2);")?;
            stmt.bind_i64(1, trace.id)?;
            stmt.bind_text(2, &encode_map(&trace.payload))?;
            stmt.step_done()?;
            self.append_op("add_trace", trace.id, "trace")?;
            Ok(())
        })();
        self.finish_transaction(result)
    }

    pub(crate) fn load_nodes(&self) -> Result<Vec<NodeRecord>, String> {
        let mut stmt =
            self.prepare("SELECT id, external_id, labels, properties FROM nodes ORDER BY id ASC;")?;
        let mut nodes = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => nodes.push(NodeRecord {
                    id: stmt.column_i64(0)?,
                    external_id: stmt.column_text(1)?,
                    labels: decode_list(&stmt.column_text(2)?),
                    properties: decode_map(&stmt.column_text(3)?)?,
                }),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        Ok(nodes)
    }

    pub(crate) fn load_edges(&self) -> Result<Vec<EdgeRecord>, String> {
        let mut stmt = self.prepare(
            "SELECT id, source, target, edge_type, properties FROM edges ORDER BY id ASC;",
        )?;
        let mut edges = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => edges.push(EdgeRecord {
                    id: stmt.column_i64(0)?,
                    source: stmt.column_i64(1)?,
                    target: stmt.column_i64(2)?,
                    edge_type: stmt.column_text(3)?,
                    properties: decode_map(&stmt.column_text(4)?)?,
                }),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        Ok(edges)
    }

    pub(crate) fn load_variables(&self) -> Result<Vec<VariableRecord>, String> {
        let mut stmt = self.prepare(
            "SELECT id, owner_id, domain, prior, posterior FROM variables ORDER BY id ASC;",
        )?;
        let mut variables = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => variables.push(VariableRecord {
                    id: stmt.column_i64(0)?,
                    owner_id: stmt.column_optional_i64(1)?,
                    domain: stmt.column_text(2)?,
                    states: Vec::new(),
                    prior: decode_map(&stmt.column_text(3)?)?,
                    posterior: decode_map(&stmt.column_text(4)?)?,
                }),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        for variable in &mut variables {
            variable.states = self.load_variable_states(variable.id, &variable.domain)?;
        }
        Ok(variables)
    }

    pub(crate) fn load_factors(&self) -> Result<Vec<FactorRecord>, String> {
        let mut stmt = self.prepare(
            "SELECT id, input_variables, output_variables, function, parameters FROM factors ORDER BY id ASC;",
        )?;
        let mut factors = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => factors.push(FactorRecord {
                    id: stmt.column_i64(0)?,
                    input_variables: decode_u64_list(&stmt.column_text(1)?)?,
                    output_variables: decode_u64_list(&stmt.column_text(2)?)?,
                    function: stmt.column_text(3)?,
                    parameters: decode_map(&stmt.column_text(4)?)?,
                }),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        Ok(factors)
    }

    pub(crate) fn load_factor_tables(&self) -> Result<Vec<FactorTableRecord>, String> {
        let mut stmt = self.prepare(
            "SELECT factor_id, variables, values_text, is_cpd FROM factor_tables ORDER BY factor_id ASC;",
        )?;
        let mut factor_tables = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => factor_tables.push(FactorTableRecord {
                    factor_id: stmt.column_i64(0)?,
                    variables: decode_u64_list(&stmt.column_text(1)?)?,
                    values: decode_f64_list(&stmt.column_text(2)?)?,
                    is_cpd: stmt.column_i64(3)? != 0,
                }),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        Ok(factor_tables)
    }

    pub(crate) fn load_posteriors(&self) -> Result<Vec<(u64, Vec<f64>)>, String> {
        let mut stmt = self.prepare(
            "SELECT variable_id, values_text FROM latest_posteriors ORDER BY variable_id ASC;",
        )?;
        let mut posteriors = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => {
                    posteriors.push((stmt.column_i64(0)?, decode_f64_list(&stmt.column_text(1)?)?))
                }
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        Ok(posteriors)
    }

    pub(crate) fn upsert_posterior(&self, variable_id: u64, values: &[f64]) -> Result<(), String> {
        let mut stmt = self.prepare(
            "INSERT INTO latest_posteriors (variable_id, values_text) VALUES (?1, ?2)
             ON CONFLICT(variable_id) DO UPDATE SET values_text = excluded.values_text;",
        )?;
        stmt.bind_i64(1, variable_id)?;
        stmt.bind_text(2, &encode_f64_list(values))?;
        stmt.step_done()
    }

    pub(crate) fn load_evidence(&self) -> Result<Vec<EvidenceRecord>, String> {
        let mut stmt =
            self.prepare("SELECT id, variable_id, payload FROM evidence ORDER BY id ASC;")?;
        let mut evidence = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => evidence.push(EvidenceRecord {
                    id: stmt.column_i64(0)?,
                    variable_id: stmt.column_i64(1)?,
                    payload: decode_map(&stmt.column_text(2)?)?,
                }),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        Ok(evidence)
    }

    pub(crate) fn load_traces(&self) -> Result<Vec<TraceRecord>, String> {
        let mut stmt = self.prepare("SELECT id, payload FROM traces ORDER BY id ASC;")?;
        let mut traces = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => traces.push(TraceRecord {
                    id: stmt.column_i64(0)?,
                    payload: decode_map(&stmt.column_text(1)?)?,
                }),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        Ok(traces)
    }

    pub(crate) fn load_segment(
        &self,
        expected_nodes: usize,
        expected_edges: usize,
    ) -> Result<Option<ComputeSegment>, String> {
        let manifest_path = self.segment_manifest_path();
        if !manifest_path.exists() {
            return Ok(None);
        }
        let Ok(manifest) = fs::read_to_string(&manifest_path) else {
            return Ok(None);
        };
        let Some(segment_file) =
            parse_segment_manifest(&manifest, expected_nodes, expected_edges).unwrap_or(None)
        else {
            return Ok(None);
        };
        let segment_path = self.segment_dir().join(segment_file);
        let Ok(bytes) = fs::read(&segment_path) else {
            return Ok(None);
        };
        Ok(ComputeSegment::from_bytes(&bytes, expected_nodes, expected_edges).ok())
    }

    pub(crate) fn save_segment(
        &self,
        segment: &ComputeSegment,
        node_count: usize,
        edge_count: usize,
    ) -> Result<(), String> {
        let segment_dir = self.segment_dir();
        fs::create_dir_all(&segment_dir).map_err(|error| {
            format!(
                "failed to create segment directory {}: {error}",
                segment_dir.display()
            )
        })?;
        let segment_file = "segment-v1.bin";
        let segment_path = segment_dir.join(segment_file);
        fs::write(&segment_path, segment.to_bytes()?).map_err(|error| {
            format!(
                "failed to write segment file {}: {error}",
                segment_path.display()
            )
        })?;
        let manifest = format!(
            "version=tonggraph-segment-v1\nnode_count={node_count}\nedge_count={edge_count}\nfile={segment_file}\n"
        );
        let manifest_path = self.segment_manifest_path();
        fs::write(&manifest_path, manifest).map_err(|error| {
            format!(
                "failed to write segment manifest {}: {error}",
                manifest_path.display()
            )
        })
    }

    fn initialize(&self) -> Result<(), String> {
        self.exec(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = FULL;
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 5000;
            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS nodes (
                id INTEGER PRIMARY KEY,
                external_id TEXT NOT NULL UNIQUE,
                labels TEXT NOT NULL,
                properties TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS node_properties (
                node_id INTEGER NOT NULL,
                key TEXT NOT NULL,
                value_type TEXT NOT NULL,
                value_text TEXT NOT NULL,
                PRIMARY KEY (node_id, key)
            );
            CREATE TABLE IF NOT EXISTS edges (
                id INTEGER PRIMARY KEY,
                source INTEGER NOT NULL,
                target INTEGER NOT NULL,
                edge_type TEXT NOT NULL,
                properties TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS edge_properties (
                edge_id INTEGER NOT NULL,
                key TEXT NOT NULL,
                value_type TEXT NOT NULL,
                value_text TEXT NOT NULL,
                PRIMARY KEY (edge_id, key)
            );
            CREATE TABLE IF NOT EXISTS property_keys (
                scope TEXT NOT NULL,
                key TEXT NOT NULL,
                PRIMARY KEY (scope, key)
            );
            CREATE TABLE IF NOT EXISTS property_values (
                scope TEXT NOT NULL,
                key TEXT NOT NULL,
                value_type TEXT NOT NULL,
                value_text TEXT NOT NULL,
                PRIMARY KEY (scope, key, value_type, value_text)
            );
            CREATE TABLE IF NOT EXISTS fulltext_indexes (
                name TEXT PRIMARY KEY,
                target TEXT NOT NULL,
                properties TEXT NOT NULL,
                tokenizer TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS op_log (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                op TEXT NOT NULL,
                object_id INTEGER NOT NULL,
                payload TEXT NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            );
            CREATE TABLE IF NOT EXISTS variables (
                id INTEGER PRIMARY KEY,
                owner_id INTEGER,
                domain TEXT NOT NULL,
                prior TEXT NOT NULL,
                posterior TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS variable_states (
                variable_id INTEGER NOT NULL,
                state_index INTEGER NOT NULL,
                state TEXT NOT NULL,
                PRIMARY KEY (variable_id, state_index)
            );
            CREATE TABLE IF NOT EXISTS factors (
                id INTEGER PRIMARY KEY,
                input_variables TEXT NOT NULL,
                output_variables TEXT NOT NULL,
                function TEXT NOT NULL,
                parameters TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS factor_tables (
                factor_id INTEGER PRIMARY KEY,
                variables TEXT NOT NULL,
                values_text TEXT NOT NULL,
                is_cpd INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS latest_posteriors (
                variable_id INTEGER PRIMARY KEY,
                values_text TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS evidence (
                id INTEGER PRIMARY KEY,
                variable_id INTEGER NOT NULL,
                payload TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS traces (
                id INTEGER PRIMARY KEY,
                payload TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_nodes_external_id ON nodes(external_id);
            CREATE INDEX IF NOT EXISTS idx_node_properties_key ON node_properties(key, node_id);
            CREATE INDEX IF NOT EXISTS idx_node_properties_key_value ON node_properties(key, value_type, value_text, node_id);
            CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
            CREATE INDEX IF NOT EXISTS idx_edges_type ON edges(edge_type);
            CREATE INDEX IF NOT EXISTS idx_edge_properties_key ON edge_properties(key, edge_id);
            CREATE INDEX IF NOT EXISTS idx_edge_properties_key_value ON edge_properties(key, value_type, value_text, edge_id);
            CREATE INDEX IF NOT EXISTS idx_property_keys_scope ON property_keys(scope, key);
            CREATE INDEX IF NOT EXISTS idx_property_values_lookup ON property_values(scope, key, value_type, value_text);
            CREATE INDEX IF NOT EXISTS idx_variables_owner ON variables(owner_id);
            CREATE INDEX IF NOT EXISTS idx_variable_states_variable ON variable_states(variable_id, state_index);
            CREATE INDEX IF NOT EXISTS idx_evidence_variable ON evidence(variable_id);
            CREATE INDEX IF NOT EXISTS idx_op_log_op ON op_log(op);
            ",
        )?;
        self.upsert_metadata("storage_format", "tonggraph-sqlite-v1")?;
        self.rebuild_property_catalog()?;
        Ok(())
    }

    fn segment_dir(&self) -> PathBuf {
        PathBuf::from(format!("{}.segments", self.path.to_string_lossy()))
    }

    fn segment_manifest_path(&self) -> PathBuf {
        self.segment_dir().join("manifest.txt")
    }

    fn insert_node_property_rows(
        &self,
        node_id: u64,
        properties: &PropertyMap,
    ) -> Result<(), String> {
        let mut stmt = self.prepare(
            "INSERT OR REPLACE INTO node_properties (node_id, key, value_type, value_text)
             VALUES (?1, ?2, ?3, ?4);",
        )?;
        for (key, value) in properties {
            stmt.bind_i64(1, node_id)?;
            stmt.bind_text(2, key)?;
            stmt.bind_text(3, value.type_name())?;
            stmt.bind_text(4, &value.encoded_value())?;
            stmt.step_done()?;
            stmt.reset()?;
            self.insert_property_catalog_row(
                "node",
                key,
                value.type_name(),
                &value.encoded_value(),
            )?;
        }
        Ok(())
    }

    fn insert_node_row(&self, node: &NodeRecord) -> Result<(), String> {
        let mut stmt = self.prepare(
            "INSERT INTO nodes (id, external_id, labels, properties) VALUES (?1, ?2, ?3, ?4);",
        )?;
        stmt.bind_i64(1, node.id)?;
        stmt.bind_text(2, &node.external_id)?;
        stmt.bind_text(3, &encode_list(&node.labels))?;
        stmt.bind_text(4, &encode_map(&node.properties))?;
        stmt.step_done()?;
        self.insert_node_property_rows(node.id, &node.properties)?;
        self.append_op("add_node", node.id, &node.external_id)?;
        self.sync_fulltext_entity("node", node.id, Some(&node.properties))
    }

    fn upsert_node_row(&self, node: &NodeRecord) -> Result<(), String> {
        let mut stmt = self.prepare(
            "INSERT INTO nodes (id, external_id, labels, properties) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET external_id = excluded.external_id, labels = excluded.labels, properties = excluded.properties;",
        )?;
        stmt.bind_i64(1, node.id)?;
        stmt.bind_text(2, &node.external_id)?;
        stmt.bind_text(3, &encode_list(&node.labels))?;
        stmt.bind_text(4, &encode_map(&node.properties))?;
        stmt.step_done()?;
        let mut delete = self.prepare("DELETE FROM node_properties WHERE node_id = ?1;")?;
        delete.bind_i64(1, node.id)?;
        delete.step_done()?;
        self.insert_node_property_rows(node.id, &node.properties)?;
        self.append_op("update_node", node.id, &node.external_id)?;
        self.sync_fulltext_entity("node", node.id, Some(&node.properties))
    }

    fn upsert_edge_row(&self, edge: &EdgeRecord) -> Result<(), String> {
        let mut stmt = self.prepare(
            "INSERT INTO edges (id, source, target, edge_type, properties) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET source = excluded.source, target = excluded.target, edge_type = excluded.edge_type, properties = excluded.properties;",
        )?;
        stmt.bind_i64(1, edge.id)?;
        stmt.bind_i64(2, edge.source)?;
        stmt.bind_i64(3, edge.target)?;
        stmt.bind_text(4, &edge.edge_type)?;
        stmt.bind_text(5, &encode_map(&edge.properties))?;
        stmt.step_done()?;
        let mut delete = self.prepare("DELETE FROM edge_properties WHERE edge_id = ?1;")?;
        delete.bind_i64(1, edge.id)?;
        delete.step_done()?;
        self.insert_edge_property_rows(edge.id, &edge.properties)?;
        self.append_op("update_edge", edge.id, &edge.edge_type)?;
        self.sync_fulltext_entity("edge", edge.id, Some(&edge.properties))
    }

    fn delete_node_row(&self, node_id: u64) -> Result<(), String> {
        let mut properties = self.prepare("DELETE FROM node_properties WHERE node_id = ?1;")?;
        properties.bind_i64(1, node_id)?;
        properties.step_done()?;
        let mut node = self.prepare("DELETE FROM nodes WHERE id = ?1;")?;
        node.bind_i64(1, node_id)?;
        node.step_done()?;
        self.append_op("delete_node", node_id, "")?;
        self.sync_fulltext_entity("node", node_id, None)
    }

    fn delete_edge_row(&self, edge_id: u64) -> Result<(), String> {
        let mut properties = self.prepare("DELETE FROM edge_properties WHERE edge_id = ?1;")?;
        properties.bind_i64(1, edge_id)?;
        properties.step_done()?;
        let mut edge = self.prepare("DELETE FROM edges WHERE id = ?1;")?;
        edge.bind_i64(1, edge_id)?;
        edge.step_done()?;
        self.append_op("delete_edge", edge_id, "")?;
        self.sync_fulltext_entity("edge", edge_id, None)
    }

    fn insert_variable_state_rows(
        &self,
        variable_id: u64,
        states: &[String],
    ) -> Result<(), String> {
        let mut stmt = self.prepare(
            "INSERT OR REPLACE INTO variable_states (variable_id, state_index, state)
             VALUES (?1, ?2, ?3);",
        )?;
        for (index, state) in states.iter().enumerate() {
            stmt.bind_i64(1, variable_id)?;
            stmt.bind_i64(2, index as u64)?;
            stmt.bind_text(3, state)?;
            stmt.step_done()?;
            stmt.reset()?;
        }
        Ok(())
    }

    fn load_variable_states(&self, variable_id: u64, domain: &str) -> Result<Vec<String>, String> {
        let mut stmt = self.prepare(
            "SELECT state FROM variable_states WHERE variable_id = ?1 ORDER BY state_index ASC;",
        )?;
        stmt.bind_i64(1, variable_id)?;
        let mut states = Vec::new();
        loop {
            match stmt.step()? {
                SQLITE_ROW => states.push(stmt.column_text(0)?),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
        }
        if states.is_empty() && domain == "binary" {
            Ok(vec!["false".to_string(), "true".to_string()])
        } else {
            Ok(states)
        }
    }

    fn insert_edge_property_rows(
        &self,
        edge_id: u64,
        properties: &PropertyMap,
    ) -> Result<(), String> {
        let mut stmt = self.prepare(
            "INSERT OR REPLACE INTO edge_properties (edge_id, key, value_type, value_text)
             VALUES (?1, ?2, ?3, ?4);",
        )?;
        for (key, value) in properties {
            stmt.bind_i64(1, edge_id)?;
            stmt.bind_text(2, key)?;
            stmt.bind_text(3, value.type_name())?;
            stmt.bind_text(4, &value.encoded_value())?;
            stmt.step_done()?;
            stmt.reset()?;
            self.insert_property_catalog_row(
                "edge",
                key,
                value.type_name(),
                &value.encoded_value(),
            )?;
        }
        Ok(())
    }

    fn insert_edge_row(&self, edge: &EdgeRecord) -> Result<(), String> {
        let mut stmt = self.prepare(
            "INSERT INTO edges (id, source, target, edge_type, properties) VALUES (?1, ?2, ?3, ?4, ?5);",
        )?;
        stmt.bind_i64(1, edge.id)?;
        stmt.bind_i64(2, edge.source)?;
        stmt.bind_i64(3, edge.target)?;
        stmt.bind_text(4, &edge.edge_type)?;
        stmt.bind_text(5, &encode_map(&edge.properties))?;
        stmt.step_done()?;
        self.insert_edge_property_rows(edge.id, &edge.properties)?;
        self.append_op("add_edge", edge.id, &edge.edge_type)?;
        self.sync_fulltext_entity("edge", edge.id, Some(&edge.properties))
    }

    fn insert_property_catalog_row(
        &self,
        scope: &str,
        key: &str,
        value_type: &str,
        value_text: &str,
    ) -> Result<(), String> {
        let mut key_stmt = self.prepare(
            "INSERT OR IGNORE INTO property_keys (scope, key)
             VALUES (?1, ?2);",
        )?;
        key_stmt.bind_text(1, scope)?;
        key_stmt.bind_text(2, key)?;
        key_stmt.step_done()?;

        let mut value_stmt = self.prepare(
            "INSERT OR IGNORE INTO property_values (scope, key, value_type, value_text)
             VALUES (?1, ?2, ?3, ?4);",
        )?;
        value_stmt.bind_text(1, scope)?;
        value_stmt.bind_text(2, key)?;
        value_stmt.bind_text(3, value_type)?;
        value_stmt.bind_text(4, value_text)?;
        value_stmt.step_done()
    }

    fn reset_property_catalog(&self) -> Result<(), String> {
        self.exec(
            "DELETE FROM property_keys;
             DELETE FROM property_values;",
        )?;
        self.rebuild_property_catalog()
    }

    fn rebuild_property_catalog(&self) -> Result<(), String> {
        self.exec(
            "
            INSERT OR IGNORE INTO property_keys (scope, key)
                SELECT 'node', key FROM node_properties;
            INSERT OR IGNORE INTO property_values (scope, key, value_type, value_text)
                SELECT 'node', key, value_type, value_text FROM node_properties;
            INSERT OR IGNORE INTO property_keys (scope, key)
                SELECT 'edge', key FROM edge_properties;
            INSERT OR IGNORE INTO property_values (scope, key, value_type, value_text)
                SELECT 'edge', key, value_type, value_text FROM edge_properties;
            ",
        )
    }

    fn upsert_metadata(&self, key: &str, value: &str) -> Result<(), String> {
        let mut stmt = self.prepare(
            "INSERT INTO metadata (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
        )?;
        stmt.bind_text(1, key)?;
        stmt.bind_text(2, value)?;
        stmt.step_done()
    }

    fn append_op(&self, op: &str, object_id: u64, payload: &str) -> Result<(), String> {
        let mut stmt =
            self.prepare("INSERT INTO op_log (op, object_id, payload) VALUES (?1, ?2, ?3);")?;
        stmt.bind_text(1, op)?;
        stmt.bind_i64(2, object_id)?;
        stmt.bind_text(3, payload)?;
        stmt.step_done()
    }

    pub(crate) fn load_next_ids(&self) -> Result<(Option<u64>, Option<u64>), String> {
        Ok((
            self.load_u64_metadata("next_node_id")?,
            self.load_u64_metadata("next_edge_id")?,
        ))
    }

    fn load_u64_metadata(&self, key: &str) -> Result<Option<u64>, String> {
        let mut stmt = self.prepare("SELECT value FROM metadata WHERE key = ?1;")?;
        stmt.bind_text(1, key)?;
        match stmt.step()? {
            SQLITE_ROW => stmt
                .column_text(0)?
                .parse::<u64>()
                .map(Some)
                .map_err(|_| format!("metadata {key:?} must be an unsigned integer")),
            SQLITE_DONE => Ok(None),
            rc => Err(format!("unexpected SQLite step result {rc}")),
        }
    }

    pub(crate) fn current_op_seq(&self) -> Result<u64, String> {
        let mut stmt = self.prepare("SELECT COALESCE(MAX(seq), 0) FROM op_log;")?;
        match stmt.step()? {
            SQLITE_ROW => stmt.column_i64(0),
            SQLITE_DONE => Ok(0),
            rc => Err(format!("unexpected SQLite step result {rc}")),
        }
    }

    fn finish_transaction(&self, result: Result<(), String>) -> Result<(), String> {
        match result {
            Ok(()) => self.exec("COMMIT;"),
            Err(error) => {
                let _ = self.exec("ROLLBACK;");
                Err(error)
            }
        }
    }

    fn exec(&self, sql: &str) -> Result<(), String> {
        let sql = CString::new(sql).map_err(|_| "SQL contains NUL byte".to_string())?;
        let mut err_msg: *mut c_char = ptr::null_mut();
        let rc =
            unsafe { sqlite3_exec(self.db, sql.as_ptr(), None, ptr::null_mut(), &mut err_msg) };
        if rc == SQLITE_OK {
            return Ok(());
        }

        if !err_msg.is_null() {
            let message = unsafe { CStr::from_ptr(err_msg) }
                .to_string_lossy()
                .into_owned();
            unsafe {
                sqlite3_free(err_msg.cast());
            }
            Err(message)
        } else {
            Err(sqlite_error(self.db))
        }
    }

    fn prepare(&self, sql: &str) -> Result<Statement<'_>, String> {
        let sql = CString::new(sql).map_err(|_| "SQL contains NUL byte".to_string())?;
        let mut stmt = ptr::null_mut();
        let rc =
            unsafe { sqlite3_prepare_v2(self.db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut()) };
        if rc != SQLITE_OK {
            return Err(sqlite_error(self.db));
        }
        Ok(Statement {
            db: self.db,
            stmt,
            _store: PhantomData,
        })
    }
}

impl Drop for SqliteStore {
    fn drop(&mut self) {
        if !self.db.is_null() {
            unsafe {
                sqlite3_close(self.db);
            }
        }
    }
}

impl GraphStore for SqliteStore {
    fn insert_nodes(&self, nodes: &[NodeRecord]) -> Result<(), String> {
        Self::insert_nodes(self, nodes)
    }

    fn insert_edges(&self, edges: &[EdgeRecord]) -> Result<(), String> {
        Self::insert_edges(self, edges)
    }

    fn apply_graph_changes(&self, changes: &GraphChanges) -> Result<(), String> {
        Self::apply_graph_changes(self, changes)
    }

    fn insert_variable(&self, variable: &VariableRecord) -> Result<(), String> {
        Self::insert_variable(self, variable)
    }

    fn insert_factor(&self, factor: &FactorRecord) -> Result<(), String> {
        Self::insert_factor(self, factor)
    }

    fn insert_factor_table(&self, factor_table: &FactorTableRecord) -> Result<(), String> {
        Self::insert_factor_table(self, factor_table)
    }

    fn insert_evidence(&self, evidence: &EvidenceRecord) -> Result<(), String> {
        Self::insert_evidence(self, evidence)
    }

    fn insert_trace(&self, trace: &TraceRecord) -> Result<(), String> {
        Self::insert_trace(self, trace)
    }

    fn load_nodes(&self) -> Result<Vec<NodeRecord>, String> {
        Self::load_nodes(self)
    }

    fn load_edges(&self) -> Result<Vec<EdgeRecord>, String> {
        Self::load_edges(self)
    }

    fn load_variables(&self) -> Result<Vec<VariableRecord>, String> {
        Self::load_variables(self)
    }

    fn load_factors(&self) -> Result<Vec<FactorRecord>, String> {
        Self::load_factors(self)
    }

    fn load_factor_tables(&self) -> Result<Vec<FactorTableRecord>, String> {
        Self::load_factor_tables(self)
    }

    fn load_posteriors(&self) -> Result<Vec<(u64, Vec<f64>)>, String> {
        Self::load_posteriors(self)
    }

    fn upsert_posterior(&self, variable_id: u64, values: &[f64]) -> Result<(), String> {
        Self::upsert_posterior(self, variable_id, values)
    }

    fn load_evidence(&self) -> Result<Vec<EvidenceRecord>, String> {
        Self::load_evidence(self)
    }

    fn load_traces(&self) -> Result<Vec<TraceRecord>, String> {
        Self::load_traces(self)
    }

    fn load_segment(
        &self,
        expected_nodes: usize,
        expected_edges: usize,
    ) -> Result<Option<ComputeSegment>, String> {
        Self::load_segment(self, expected_nodes, expected_edges)
    }

    fn save_segment(
        &self,
        segment: &ComputeSegment,
        node_count: usize,
        edge_count: usize,
    ) -> Result<(), String> {
        Self::save_segment(self, segment, node_count, edge_count)
    }

    fn current_op_seq(&self) -> Result<u64, String> {
        Self::current_op_seq(self)
    }

    fn load_next_ids(&self) -> Result<(Option<u64>, Option<u64>), String> {
        Self::load_next_ids(self)
    }

    fn load_fulltext_indexes(&self) -> Result<Vec<FullTextIndexDefinition>, String> {
        Self::load_fulltext_indexes(self)
    }

    fn create_fulltext_index(
        &self,
        definition: &FullTextIndexDefinition,
        nodes: &[NodeRecord],
        edges: &[EdgeRecord],
    ) -> Result<(), String> {
        Self::create_fulltext_index(self, definition, nodes, edges)
    }

    fn drop_fulltext_index(&self, name: &str) -> Result<(), String> {
        Self::drop_fulltext_index(self, name)
    }

    fn rebuild_fulltext_indexes(
        &self,
        definitions: &[FullTextIndexDefinition],
        nodes: &[NodeRecord],
        edges: &[EdgeRecord],
    ) -> Result<(), String> {
        Self::rebuild_fulltext_indexes(self, definitions, nodes, edges)
    }

    fn fulltext_candidates(
        &self,
        definition: &FullTextIndexDefinition,
        expression: &str,
    ) -> Result<Vec<u64>, String> {
        Self::fulltext_candidates(self, definition, expression)
    }

    fn path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

struct Statement<'a> {
    db: *mut Sqlite3,
    stmt: *mut Sqlite3Stmt,
    _store: PhantomData<&'a SqliteStore>,
}

impl Statement<'_> {
    fn bind_i64(&mut self, index: c_int, value: u64) -> Result<(), String> {
        if value > i64::MAX as u64 {
            return Err(format!("value {value} exceeds SQLite INTEGER range"));
        }
        let rc = unsafe { sqlite3_bind_int64(self.stmt, index, value as i64) };
        self.check(rc)
    }

    fn bind_optional_i64(&mut self, index: c_int, value: Option<u64>) -> Result<(), String> {
        match value {
            Some(value) => self.bind_i64(index, value),
            None => {
                let rc = unsafe { sqlite3_bind_null(self.stmt, index) };
                self.check(rc)
            }
        }
    }

    fn bind_text(&mut self, index: c_int, value: &str) -> Result<(), String> {
        let value = CString::new(value).map_err(|_| "text contains NUL byte".to_string())?;
        let rc = unsafe {
            sqlite3_bind_text(
                self.stmt,
                index,
                value.as_ptr(),
                value.as_bytes().len() as c_int,
                Some(sqlite_transient()),
            )
        };
        self.check(rc)
    }

    fn step(&mut self) -> Result<c_int, String> {
        let rc = unsafe { sqlite3_step(self.stmt) };
        match rc {
            SQLITE_ROW | SQLITE_DONE => Ok(rc),
            _ => Err(sqlite_error(self.db)),
        }
    }

    fn step_done(&mut self) -> Result<(), String> {
        match self.step()? {
            SQLITE_DONE => Ok(()),
            SQLITE_ROW => Err("SQLite statement returned a row unexpectedly".to_string()),
            rc => Err(format!("unexpected SQLite step result {rc}")),
        }
    }

    fn reset(&mut self) -> Result<(), String> {
        let rc = unsafe { sqlite3_reset(self.stmt) };
        self.check(rc)
    }

    fn column_i64(&self, index: c_int) -> Result<u64, String> {
        let value = unsafe { sqlite3_column_int64(self.stmt, index) };
        u64::try_from(value).map_err(|_| format!("negative SQLite integer {value}"))
    }

    fn column_optional_i64(&self, index: c_int) -> Result<Option<u64>, String> {
        if unsafe { sqlite3_column_type(self.stmt, index) } == SQLITE_NULL {
            return Ok(None);
        }
        self.column_i64(index).map(Some)
    }

    fn column_text(&self, index: c_int) -> Result<String, String> {
        let ptr = unsafe { sqlite3_column_text(self.stmt, index) };
        if ptr.is_null() {
            return Ok(String::new());
        }
        Ok(unsafe { CStr::from_ptr(ptr.cast()) }
            .to_string_lossy()
            .into_owned())
    }

    fn check(&self, rc: c_int) -> Result<(), String> {
        if rc == SQLITE_OK {
            Ok(())
        } else {
            Err(sqlite_error(self.db))
        }
    }
}

impl Drop for Statement<'_> {
    fn drop(&mut self) {
        if !self.stmt.is_null() {
            unsafe {
                sqlite3_finalize(self.stmt);
            }
        }
    }
}

fn fulltext_table(tokenizer: &str) -> Result<&'static str, String> {
    match tokenizer {
        "unicode61" => Ok("fulltext_unicode"),
        "trigram" => Ok("fulltext_trigram"),
        _ => Err(format!("unknown full-text tokenizer {tokenizer:?}")),
    }
}

fn sqlite_error(db: *mut Sqlite3) -> String {
    if db.is_null() {
        return "SQLite database handle is null".to_string();
    }
    let ptr = unsafe { sqlite3_errmsg(db) };
    if ptr.is_null() {
        "unknown SQLite error".to_string()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

fn sqlite_transient() -> unsafe extern "C" fn(*mut c_void) {
    unsafe { std::mem::transmute::<isize, unsafe extern "C" fn(*mut c_void)>(-1) }
}

fn parse_segment_manifest(
    manifest: &str,
    expected_nodes: usize,
    expected_edges: usize,
) -> Result<Option<String>, String> {
    let mut version = None;
    let mut node_count = None;
    let mut edge_count = None;
    let mut file = None;

    for line in manifest.lines() {
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("invalid segment manifest line {line:?}"));
        };
        match key {
            "version" => version = Some(value.to_string()),
            "node_count" => {
                node_count = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("invalid segment manifest node_count {value:?}"))?,
                );
            }
            "edge_count" => {
                edge_count = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| format!("invalid segment manifest edge_count {value:?}"))?,
                );
            }
            "file" => file = Some(value.to_string()),
            _ => {}
        }
    }

    if version.as_deref() != Some("tonggraph-segment-v1") {
        return Ok(None);
    }
    if node_count != Some(expected_nodes) || edge_count != Some(expected_edges) {
        return Ok(None);
    }
    let file = file.ok_or_else(|| "segment manifest is missing file".to_string())?;
    if file.contains('/') || file.contains('\\') || file.is_empty() {
        return Err(format!("invalid segment manifest file {file:?}"));
    }
    Ok(Some(file))
}
