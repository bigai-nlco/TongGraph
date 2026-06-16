use crate::codec::{
    decode_list, decode_map, decode_u64_list, encode_list, encode_map, encode_u64_list,
};
use crate::core::segment::ComputeSegment;
use crate::models::{
    EdgeRecord, EvidenceRecord, FactorRecord, NodeRecord, PropertyMap, TraceRecord, VariableRecord,
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
    fn insert_node(&self, node: &NodeRecord) -> Result<(), String>;
    fn insert_edge(&self, edge: &EdgeRecord) -> Result<(), String>;
    fn insert_variable(&self, variable: &VariableRecord) -> Result<(), String>;
    fn insert_factor(&self, factor: &FactorRecord) -> Result<(), String>;
    fn insert_evidence(&self, evidence: &EvidenceRecord) -> Result<(), String>;
    fn insert_trace(&self, trace: &TraceRecord) -> Result<(), String>;
    fn load_nodes(&self) -> Result<Vec<NodeRecord>, String>;
    fn load_edges(&self) -> Result<Vec<EdgeRecord>, String>;
    fn load_variables(&self) -> Result<Vec<VariableRecord>, String>;
    fn load_factors(&self) -> Result<Vec<FactorRecord>, String>;
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

    pub(crate) fn insert_node(&self, node: &NodeRecord) -> Result<(), String> {
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
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
            Ok(())
        })();
        self.finish_transaction(result)
    }

    pub(crate) fn insert_edge(&self, edge: &EdgeRecord) -> Result<(), String> {
        self.exec("BEGIN IMMEDIATE;")?;
        let result = (|| {
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
            Ok(())
        })();
        self.finish_transaction(result)
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
                    prior: decode_map(&stmt.column_text(3)?)?,
                    posterior: decode_map(&stmt.column_text(4)?)?,
                }),
                SQLITE_DONE => break,
                rc => return Err(format!("unexpected SQLite step result {rc}")),
            }
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
        let manifest = fs::read_to_string(&manifest_path).map_err(|error| {
            format!(
                "failed to read segment manifest {}: {error}",
                manifest_path.display()
            )
        })?;
        let Some(segment_file) = parse_segment_manifest(&manifest, expected_nodes, expected_edges)?
        else {
            return Ok(None);
        };
        let segment_path = self.segment_dir().join(segment_file);
        let bytes = fs::read(&segment_path).map_err(|error| {
            format!(
                "failed to read segment file {}: {error}",
                segment_path.display()
            )
        })?;
        ComputeSegment::from_bytes(&bytes, expected_nodes, expected_edges).map(Some)
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
            PRAGMA synchronous = NORMAL;
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
            CREATE TABLE IF NOT EXISTS factors (
                id INTEGER PRIMARY KEY,
                input_variables TEXT NOT NULL,
                output_variables TEXT NOT NULL,
                function TEXT NOT NULL,
                parameters TEXT NOT NULL
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
    fn insert_node(&self, node: &NodeRecord) -> Result<(), String> {
        Self::insert_node(self, node)
    }

    fn insert_edge(&self, edge: &EdgeRecord) -> Result<(), String> {
        Self::insert_edge(self, edge)
    }

    fn insert_variable(&self, variable: &VariableRecord) -> Result<(), String> {
        Self::insert_variable(self, variable)
    }

    fn insert_factor(&self, factor: &FactorRecord) -> Result<(), String> {
        Self::insert_factor(self, factor)
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
