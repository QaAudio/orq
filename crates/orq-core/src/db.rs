//! Database compatibility layer: one sync API over two backends.
//!
//! - `Local` — rusqlite on a plain file (default, offline-first, WAL).
//! - `Remote` — libsql/hrana over HTTP to a Turso (or any libSQL) primary.
//!
//! The API mirrors the small rusqlite surface the store uses (`execute`,
//! `prepare`/`query_map`, `query_row` + `optional`, `params!`), so store code
//! is backend-agnostic. Remote calls are bridged with a dedicated
//! current-thread tokio runtime owned by the connection (never the caller's).

use std::fmt;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

// ---------- values ----------

#[derive(Debug, Clone, PartialEq)]
pub enum DbValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

// ---------- errors ----------

#[derive(Debug)]
pub enum DbError {
    /// query_row found no row (mirrors rusqlite::Error::QueryReturnedNoRows).
    NoRows,
    Msg(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::NoRows => write!(f, "query returned no rows"),
            DbError::Msg(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for DbError {}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::QueryReturnedNoRows => DbError::NoRows,
            other => DbError::Msg(other.to_string()),
        }
    }
}

impl From<libsql::Error> for DbError {
    fn from(e: libsql::Error) -> Self {
        DbError::Msg(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DbError>;

/// rusqlite-style `.optional()` on query_row results.
pub trait OptionalExtension<T> {
    fn optional(self) -> Result<Option<T>>;
}

impl<T> OptionalExtension<T> for Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(DbError::NoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ---------- params ----------

pub trait ToDbValue {
    fn to_db_value(&self) -> DbValue;
}

impl<T: ToDbValue + ?Sized> ToDbValue for &T {
    fn to_db_value(&self) -> DbValue {
        (**self).to_db_value()
    }
}

impl ToDbValue for str {
    fn to_db_value(&self) -> DbValue {
        DbValue::Text(self.to_string())
    }
}

impl ToDbValue for String {
    fn to_db_value(&self) -> DbValue {
        DbValue::Text(self.clone())
    }
}

macro_rules! int_to_value {
    ($($t:ty),+) => {
        $(impl ToDbValue for $t {
            fn to_db_value(&self) -> DbValue { DbValue::Integer(*self as i64) }
        })+
    };
}
int_to_value!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

impl ToDbValue for f64 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Real(*self)
    }
}

impl ToDbValue for f32 {
    fn to_db_value(&self) -> DbValue {
        DbValue::Real(*self as f64)
    }
}

impl ToDbValue for bool {
    fn to_db_value(&self) -> DbValue {
        DbValue::Integer(*self as i64)
    }
}

impl<T: ToDbValue> ToDbValue for Option<T> {
    fn to_db_value(&self) -> DbValue {
        match self {
            Some(v) => v.to_db_value(),
            None => DbValue::Null,
        }
    }
}

#[macro_export]
macro_rules! __db_params {
    () => { Vec::<$crate::db::DbValue>::new() };
    ($($x:expr),+ $(,)?) => {
        vec![$($crate::db::ToDbValue::to_db_value(&$x)),+]
    };
}

pub use crate::__db_params as params;

pub trait IntoParams {
    fn into_params(self) -> Vec<DbValue>;
}

impl IntoParams for Vec<DbValue> {
    fn into_params(self) -> Vec<DbValue> {
        self
    }
}

impl IntoParams for () {
    fn into_params(self) -> Vec<DbValue> {
        Vec::new()
    }
}

// ---------- rows ----------

pub trait FromDbValue: Sized {
    fn from_db(v: &DbValue) -> Result<Self>;
}

impl FromDbValue for String {
    fn from_db(v: &DbValue) -> Result<Self> {
        match v {
            DbValue::Text(s) => Ok(s.clone()),
            DbValue::Integer(i) => Ok(i.to_string()),
            DbValue::Real(r) => Ok(r.to_string()),
            DbValue::Null => Err(DbError::Msg("unexpected NULL for String".into())),
            DbValue::Blob(_) => Err(DbError::Msg("blob to String".into())),
        }
    }
}

macro_rules! int_from_value {
    ($($t:ty),+) => {
        $(impl FromDbValue for $t {
            fn from_db(v: &DbValue) -> Result<Self> {
                match v {
                    DbValue::Integer(i) => Ok(*i as $t),
                    DbValue::Real(r) => Ok(*r as $t),
                    DbValue::Null => Err(DbError::Msg("unexpected NULL for integer".into())),
                    other => Err(DbError::Msg(format!("cannot read integer from {other:?}"))),
                }
            }
        })+
    };
}
int_from_value!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

impl FromDbValue for f64 {
    fn from_db(v: &DbValue) -> Result<Self> {
        match v {
            DbValue::Real(r) => Ok(*r),
            DbValue::Integer(i) => Ok(*i as f64),
            _ => Err(DbError::Msg("cannot read f64".into())),
        }
    }
}

impl FromDbValue for bool {
    fn from_db(v: &DbValue) -> Result<Self> {
        Ok(matches!(v, DbValue::Integer(i) if *i != 0))
    }
}

impl<T: FromDbValue> FromDbValue for Option<T> {
    fn from_db(v: &DbValue) -> Result<Self> {
        match v {
            DbValue::Null => Ok(None),
            other => T::from_db(other).map(Some),
        }
    }
}

pub trait RowIndex {
    fn idx(&self) -> usize;
}

impl RowIndex for usize {
    fn idx(&self) -> usize {
        *self
    }
}

/// A fully-materialized result row (backend-agnostic).
pub struct Row<'a> {
    vals: Vec<DbValue>,
    _p: PhantomData<&'a ()>,
}

impl Row<'_> {
    pub fn get<I: RowIndex, T: FromDbValue>(&self, i: I) -> Result<T> {
        let idx = i.idx();
        let v = self
            .vals
            .get(idx)
            .ok_or_else(|| DbError::Msg(format!("column index {idx} out of range")))?;
        T::from_db(v)
    }
}

// ---------- connection ----------

enum Inner {
    Local(rusqlite::Connection),
    Remote {
        conn: libsql::Connection,
        rt: Arc<tokio::runtime::Runtime>,
        _db: libsql::Database,
    },
}

pub struct Connection {
    inner: Inner,
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            Inner::Local(_) => write!(f, "Connection(local)"),
            Inner::Remote { .. } => write!(f, "Connection(remote)"),
        }
    }
}

/// Drive a remote future to completion on a dedicated big-stack thread.
/// The libsql SQL parser and rustls handshakes recurse deeply enough to
/// overflow the 1 MiB Windows main-thread stack; scoped threads let us
/// borrow non-'static futures while giving them 8 MiB.
fn run_remote<T: Send>(
    rt: &tokio::runtime::Runtime,
    fut: impl std::future::Future<Output = T> + Send,
) -> T {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .name("orq-db-remote".into())
            .stack_size(8 * 1024 * 1024)
            .spawn_scoped(s, || rt.block_on(fut))
            .expect("spawn db thread")
            .join()
            .expect("db thread panicked")
    })
}

fn to_rusqlite(vals: Vec<DbValue>) -> Vec<rusqlite::types::Value> {
    vals.into_iter()
        .map(|v| match v {
            DbValue::Null => rusqlite::types::Value::Null,
            DbValue::Integer(i) => rusqlite::types::Value::Integer(i),
            DbValue::Real(r) => rusqlite::types::Value::Real(r),
            DbValue::Text(s) => rusqlite::types::Value::Text(s),
            DbValue::Blob(b) => rusqlite::types::Value::Blob(b),
        })
        .collect()
}

fn to_libsql(vals: Vec<DbValue>) -> Vec<libsql::Value> {
    vals.into_iter()
        .map(|v| match v {
            DbValue::Null => libsql::Value::Null,
            DbValue::Integer(i) => libsql::Value::Integer(i),
            DbValue::Real(r) => libsql::Value::Real(r),
            DbValue::Text(s) => libsql::Value::Text(s),
            DbValue::Blob(b) => libsql::Value::Blob(b),
        })
        .collect()
}

fn from_rusqlite_ref(v: rusqlite::types::ValueRef<'_>) -> DbValue {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => DbValue::Null,
        ValueRef::Integer(i) => DbValue::Integer(i),
        ValueRef::Real(r) => DbValue::Real(r),
        ValueRef::Text(t) => DbValue::Text(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => DbValue::Blob(b.to_vec()),
    }
}

fn from_libsql(v: libsql::Value) -> DbValue {
    match v {
        libsql::Value::Null => DbValue::Null,
        libsql::Value::Integer(i) => DbValue::Integer(i),
        libsql::Value::Real(r) => DbValue::Real(r),
        libsql::Value::Text(s) => DbValue::Text(s),
        libsql::Value::Blob(b) => DbValue::Blob(b),
    }
}

impl Connection {
    pub fn open_local(path: &Path) -> Result<Self> {
        let conn = rusqlite::Connection::open(path).map_err(DbError::from)?;
        Ok(Self {
            inner: Inner::Local(conn),
        })
    }

    pub fn open_remote(url: &str, token: &str) -> Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| DbError::Msg(format!("runtime: {e}")))?;
        let (db, conn) = run_remote(&rt, async {
            let db = libsql::Builder::new_remote(url.to_string(), token.to_string())
                .build()
                .await?;
            let conn = db.connect()?;
            Ok::<_, libsql::Error>((db, conn))
        })?;
        Ok(Self {
            inner: Inner::Remote {
                conn,
                rt: Arc::new(rt),
                _db: db,
            },
        })
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.inner, Inner::Remote { .. })
    }

    pub fn execute(&self, sql: &str, params: impl IntoParams) -> Result<usize> {
        let vals = params.into_params();
        match &self.inner {
            Inner::Local(c) => {
                let n = c
                    .execute(sql, rusqlite::params_from_iter(to_rusqlite(vals)))
                    .map_err(DbError::from)?;
                Ok(n)
            }
            Inner::Remote { conn, rt, .. } => {
                let n = run_remote(rt, conn.execute(sql, to_libsql(vals)))
                    .map_err(DbError::from)?;
                Ok(n as usize)
            }
        }
    }

    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        match &self.inner {
            Inner::Local(c) => c.execute_batch(sql).map_err(DbError::from),
            Inner::Remote { conn, rt, .. } => {
                run_remote(rt, conn.execute_batch(sql)).map_err(DbError::from)?;
                Ok(())
            }
        }
    }

    pub fn prepare(&self, sql: &str) -> Result<Statement<'_>> {
        Ok(Statement {
            conn: self,
            sql: sql.to_string(),
        })
    }

    pub fn query_row<T, F>(&self, sql: &str, params: impl IntoParams, f: F) -> Result<T>
    where
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        let rows = self.query_all(sql, params.into_params())?;
        match rows.into_iter().next() {
            Some(vals) => f(&Row {
                vals,
                _p: PhantomData,
            }),
            None => Err(DbError::NoRows),
        }
    }

    pub fn last_insert_rowid(&self) -> i64 {
        match &self.inner {
            Inner::Local(c) => c.last_insert_rowid(),
            Inner::Remote { conn, .. } => conn.last_insert_rowid(),
        }
    }

    /// BEGIN a transaction (IMMEDIATE locally; plain BEGIN on remote hrana).
    pub fn begin(&self) -> Result<()> {
        match &self.inner {
            Inner::Local(_) => self.execute_batch("BEGIN IMMEDIATE"),
            Inner::Remote { .. } => self.execute_batch("BEGIN"),
        }
    }

    pub fn commit(&self) -> Result<()> {
        self.execute_batch("COMMIT")
    }

    pub fn rollback(&self) -> Result<()> {
        self.execute_batch("ROLLBACK")
    }

    fn query_all(&self, sql: &str, vals: Vec<DbValue>) -> Result<Vec<Vec<DbValue>>> {
        match &self.inner {
            Inner::Local(c) => {
                let mut stmt = c.prepare(sql).map_err(DbError::from)?;
                let ncols = stmt.column_count();
                let mut rows = stmt
                    .query(rusqlite::params_from_iter(to_rusqlite(vals)))
                    .map_err(DbError::from)?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().map_err(DbError::from)? {
                    let mut vals = Vec::with_capacity(ncols);
                    for i in 0..ncols {
                        let vref = row.get_ref(i).map_err(DbError::from)?;
                        vals.push(from_rusqlite_ref(vref));
                    }
                    out.push(vals);
                }
                Ok(out)
            }
            Inner::Remote { conn, rt, .. } => run_remote(rt, async {
                let mut rows = conn
                    .query(sql, to_libsql(vals))
                    .await
                    .map_err(DbError::from)?;
                let ncols = rows.column_count() as usize;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await.map_err(DbError::from)? {
                    let mut vals = Vec::with_capacity(ncols);
                    for i in 0..ncols {
                        let v = row.get_value(i as i32).map_err(DbError::from)?;
                        vals.push(from_libsql(v));
                    }
                    out.push(vals);
                }
                Ok(out)
            }),
        }
    }
}

/// Lazy statement: executes on `query_map` (mirrors the rusqlite call shape).
pub struct Statement<'c> {
    conn: &'c Connection,
    sql: String,
}

impl Statement<'_> {
    pub fn query_map<T, F>(
        &mut self,
        params: impl IntoParams,
        mut f: F,
    ) -> Result<std::vec::IntoIter<Result<T>>>
    where
        F: FnMut(&Row<'_>) -> Result<T>,
    {
        let rows = self.conn.query_all(&self.sql, params.into_params())?;
        let mapped: Vec<Result<T>> = rows
            .into_iter()
            .map(|vals| {
                f(&Row {
                    vals,
                    _p: PhantomData,
                })
            })
            .collect();
        Ok(mapped.into_iter())
    }
}
