use crate::codec::{decode_list, decode_map, encode_list, encode_map};
use crate::models::{EdgeRecord, NodeRecord};
use std::ffi::{c_char, c_int, c_uchar, c_void, CStr, CString};
use std::marker::PhantomData;
use std::ptr;

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;

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
    fn sqlite3_step(stmt: *mut Sqlite3Stmt) -> c_int;
    fn sqlite3_bind_int64(stmt: *mut Sqlite3Stmt, index: c_int, value: i64) -> c_int;
    fn sqlite3_bind_text(
        stmt: *mut Sqlite3Stmt,
        index: c_int,
        value: *const c_char,
        n: c_int,
        destructor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;
    fn sqlite3_column_int64(stmt: *mut Sqlite3Stmt, index: c_int) -> i64;
    fn sqlite3_column_text(stmt: *mut Sqlite3Stmt, index: c_int) -> *const c_uchar;
}

pub(crate) struct SqliteStore {
    db: *mut Sqlite3,
}

impl SqliteStore {
    pub(crate) fn open(path: &str) -> Result<Self, String> {
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

        let store = Self { db };
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
            self.append_op("add_edge", edge.id, &edge.edge_type)?;
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
            CREATE TABLE IF NOT EXISTS edges (
                id INTEGER PRIMARY KEY,
                source INTEGER NOT NULL,
                target INTEGER NOT NULL,
                edge_type TEXT NOT NULL,
                properties TEXT NOT NULL
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
            CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source);
            CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target);
            CREATE INDEX IF NOT EXISTS idx_edges_type ON edges(edge_type);
            CREATE INDEX IF NOT EXISTS idx_op_log_op ON op_log(op);
            ",
        )?;
        self.upsert_metadata("storage_format", "tonggraph-sqlite-v1")?;
        Ok(())
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

    fn column_i64(&self, index: c_int) -> Result<u64, String> {
        let value = unsafe { sqlite3_column_int64(self.stmt, index) };
        u64::try_from(value).map_err(|_| format!("negative SQLite integer {value}"))
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
