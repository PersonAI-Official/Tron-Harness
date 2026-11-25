//! Composition bridge for synchronous business services and Framework storage.
//! Entire consistency operations run in one future so transaction binding is retained.
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use eliteforge_agent_official_plugin_storage_local::SqliteDatabase;

pub(crate) fn blocking<T: Send>(future: impl Future<Output = T> + Send) -> T {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("storage runtime")
    });
    use opentelemetry::trace::FutureExt;
    let future = future.with_context(opentelemetry::Context::current());
    // The dedicated OS thread is required: the calling thread can be inside
    // an arbitrary runtime context (including a current-thread test runtime
    // or a main-runtime worker), and entering this runtime's block_on there
    // panics. The thread is cheap relative to the storage round trip.
    let execute = || {
        std::thread::scope(|scope| {
            scope
                .spawn(|| runtime.block_on(future))
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
        })
    };
    if tokio::runtime::Handle::try_current()
        .is_ok_and(|handle| handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread)
    {
        tokio::task::block_in_place(execute)
    } else {
        execute()
    }
}

pub(crate) fn chat_error(
    error: eliteforge_agent_core::StateError,
) -> crate::domain::chat::ChatError {
    use crate::domain::chat::ChatError;
    use eliteforge_agent_core::StateError;
    match error {
        StateError::NotFound => ChatError::SessionNotFound,
        StateError::Busy => ChatError::SessionRunActive,
        other => {
            tracing::error!("Framework session storage failed: {other}");
            ChatError::StorageFailure
        }
    }
}

#[derive(Clone)]
pub(crate) struct SharedDatabase(Arc<SqliteDatabase>);

/// 进程内已打开数据库的弱引用登记表：供周期性空闲页回收遍历
type DatabaseRegistry = Mutex<BTreeMap<String, Weak<SqliteDatabase>>>;

static OPTIMIZING: AtomicBool = AtomicBool::new(false);

pub(crate) fn is_optimizing() -> bool {
    OPTIMIZING.load(Ordering::Relaxed)
}

fn database_registry() -> &'static DatabaseRegistry {
    static DATABASES: OnceLock<DatabaseRegistry> = OnceLock::new();
    DATABASES.get_or_init(Default::default)
}

impl SharedDatabase {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        if path == ":memory:" {
            let database = blocking(SqliteDatabase::open(path)).map(|db| Self(Arc::new(db)))?;
            database.call(
                |c| super::schema::initialize_business(c),
                || rusqlite::Error::InvalidQuery,
            )?;
            return Ok(database);
        }
        let key = if path.starts_with("file:") {
            path.to_owned()
        } else {
            let absolute =
                std::path::absolute(path).map_err(|_| rusqlite::Error::InvalidPath(path.into()))?;
            let resolved = if absolute.exists() {
                std::fs::canonicalize(&absolute)
            } else {
                let parent = absolute
                    .parent()
                    .ok_or_else(|| rusqlite::Error::InvalidPath(absolute.clone()))?;
                std::fs::create_dir_all(parent)
                    .map_err(|_| rusqlite::Error::InvalidPath(absolute.clone()))?;
                std::fs::canonicalize(parent)
                    .map(|parent| parent.join(absolute.file_name().unwrap()))
            }
            .map_err(|_| rusqlite::Error::InvalidPath(absolute))?;
            resolved.to_string_lossy().into_owned()
        };
        let mut databases = database_registry()
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        if let Some(database) = databases.get(&key).and_then(Weak::upgrade) {
            return Ok(Self(database));
        }
        let database = Arc::new(blocking(SqliteDatabase::open(&key))?);
        let shared = Self(database.clone());
        shared.call(
            |c| super::schema::initialize_business(c),
            || rusqlite::Error::InvalidQuery,
        )?;
        databases.insert(key, Arc::downgrade(&database));
        Ok(shared)
    }

    /// 增量回收空闲页（仅 auto_vacuum=INCREMENTAL 生效），返回实际回收页数
    pub fn reclaim_free_pages(&self, max_pages: u64) -> rusqlite::Result<u64> {
        self.call(
            move |c| reclaim_free_pages_on(c, max_pages),
            || rusqlite::Error::InvalidQuery,
        )
    }
    pub fn runtime(&self) -> eliteforge_agent_official_plugin_storage_local::SqliteDatabase {
        self.0.as_ref().clone()
    }
    /// Retire the application connection before closing it, so a later profile
    /// activation cannot obtain a closed handle from the process registry.
    pub(crate) async fn close(&self) -> Result<(), tokio_rusqlite::Error> {
        database_registry()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|_, handle| {
                handle
                    .upgrade()
                    .is_some_and(|open| !Arc::ptr_eq(&open, &self.0))
            });
        self.runtime().close().await
    }
    #[track_caller]
    pub fn call<T: Send + 'static, E: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut rusqlite::Connection) -> Result<T, E> + Send + 'static,
        failure: impl FnOnce() -> E,
    ) -> Result<T, E> {
        self.tracked_call(operation, |boxed| self.0.call_blocking(boxed), failure)
    }
    /// Read-only queries run on the WAL reader pool instead of queueing
    /// behind the serialized write connection. Falls back to `call` inside a
    /// transaction or for in-memory databases.
    #[track_caller]
    pub fn call_read<T: Send + 'static, E: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut rusqlite::Connection) -> Result<T, E> + Send + 'static,
        failure: impl FnOnce() -> E,
    ) -> Result<T, E> {
        self.tracked_call(operation, |boxed| self.0.call_read_blocking(boxed), failure)
    }
    fn tracked_call<T: Send + 'static, E: Send + 'static>(
        &self,
        operation: impl FnOnce(&mut rusqlite::Connection) -> Result<T, E> + Send + 'static,
        invoke: impl FnOnce(
            Box<dyn FnOnce(&mut rusqlite::Connection) -> Result<T, E> + Send + 'static>,
        ) -> Result<T, tokio_rusqlite::Error<E>>,
        failure: impl FnOnce() -> E,
    ) -> Result<T, E> {
        let caller = std::panic::Location::caller();
        // Closure types retain the repository method, unlike a shared generic span name.
        let operation_name =
            std::any::type_name_of_val(&operation).trim_end_matches("::{{closure}}");
        let short_name = operation_name
            .split("::sqlite::")
            .last()
            .unwrap_or(operation_name);
        let span_name = format!("db.sqlite.{short_name}");
        let attributes = vec![
            opentelemetry::KeyValue::new("code.function.name", operation_name.to_owned()),
            opentelemetry::KeyValue::new("db.system.name", "sqlite"),
            opentelemetry::KeyValue::new("code.file.path", caller.file()),
            opentelemetry::KeyValue::new("code.line.number", i64::from(caller.line())),
        ];
        let parent = opentelemetry::Context::current();
        let worker_context = parent.clone();
        let started = std::time::SystemTime::now();
        let started_instant = std::time::Instant::now();
        let result = invoke(Box::new(move |connection| {
            let _guard = worker_context.attach();
            operation(connection)
        }))
        .map_err(|error| match error {
            tokio_rusqlite::Error::Error(error) => error,
            _ => failure(),
        });
        let failed = result.is_err();
        if failed || started_instant.elapsed() >= std::time::Duration::from_millis(25) {
            crate::app::telemetry::completed_operation(
                &span_name,
                &parent,
                started,
                std::time::SystemTime::now(),
                attributes,
                failed,
            );
        }
        result
    }
}

/// 增量回收空闲页；注意 incremental_vacuum 每回收一页返回一行，必须 step 到底
fn reclaim_free_pages_on(
    connection: &mut rusqlite::Connection,
    max_pages: u64,
) -> rusqlite::Result<u64> {
    let mode: i64 = connection.query_row("PRAGMA auto_vacuum", [], |row| row.get(0))?;
    if mode != 2 || max_pages == 0 {
        return Ok(0);
    }
    let freelist: i64 = connection.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
    if freelist <= 0 {
        return Ok(0);
    }
    let limit = (max_pages as i64).min(freelist);
    let mut stmt = connection.prepare(&format!("PRAGMA incremental_vacuum({limit})"))?;
    let mut rows = stmt.query([])?;
    while rows.next()?.is_some() {}
    drop(rows);
    drop(stmt);
    let after: i64 = connection.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
    Ok((freelist - after).max(0) as u64)
}

/// 遍历进程内仍存活的数据库，各回收一轮空闲页（供后台巡检调用）
pub(crate) fn reclaim_all_databases(max_pages: u64) {
    let databases: Vec<Arc<SqliteDatabase>> = {
        let mut registry = match database_registry().lock() {
            Ok(registry) => registry,
            Err(_) => return,
        };
        registry.retain(|_, weak| weak.strong_count() > 0);
        registry.values().filter_map(Weak::upgrade).collect()
    };
    for database in databases {
        let shared = SharedDatabase(database);
        if let Err(error) = shared.call(enable_incremental_reclamation, || {
            rusqlite::Error::InvalidQuery
        }) {
            tracing::warn!(%error, "SQLite auto-vacuum conversion deferred");
            continue;
        }
        if let Err(error) = shared.reclaim_free_pages(max_pages) {
            tracing::warn!(%error, "SQLite 空闲页回收失败");
        }
    }
}

fn enable_incremental_reclamation(connection: &mut rusqlite::Connection) -> rusqlite::Result<()> {
    let mode: i64 = connection.query_row("PRAGMA auto_vacuum", [], |row| row.get(0))?;
    if mode == 2 {
        return Ok(());
    }
    if mode == 1 {
        return connection.pragma_update(None, "auto_vacuum", "INCREMENTAL");
    }

    let file = std::path::Path::new(connection.path().ok_or(rusqlite::Error::InvalidQuery)?);
    let parent = file.parent().ok_or(rusqlite::Error::InvalidQuery)?;
    let file_bytes = std::fs::metadata(file)
        .map_err(|_| rusqlite::Error::InvalidPath(file.into()))?
        .len();
    let available =
        fs2::available_space(parent).map_err(|_| rusqlite::Error::InvalidPath(parent.into()))?;
    if available < file_bytes.saturating_mul(2).saturating_add(8 * 1024 * 1024) {
        tracing::warn!(
            file_bytes,
            available,
            "Insufficient space for SQLite auto-vacuum conversion"
        );
        return Ok(());
    }

    struct OptimizationGuard;
    impl Drop for OptimizationGuard {
        fn drop(&mut self) {
            OPTIMIZING.store(false, Ordering::Relaxed);
        }
    }

    OPTIMIZING.store(true, Ordering::Relaxed);
    let _guard = OptimizationGuard;
    tracing::info!(file_bytes, "Starting SQLite auto-vacuum conversion");
    connection.pragma_update(None, "auto_vacuum", "INCREMENTAL")?;
    connection.execute_batch("VACUUM")?;
    tracing::info!("SQLite auto-vacuum conversion completed");
    Ok(())
}

/// A business transaction joins the host transaction on the worker connection.
/// Its successful inner completion releases a savepoint, never the outer commit.
pub(crate) struct BusinessTransaction<'a> {
    connection: &'a rusqlite::Connection,
    savepoint: Option<String>,
    finished: bool,
}
pub(crate) fn business_transaction(
    connection: &rusqlite::Connection,
    behavior: rusqlite::TransactionBehavior,
) -> rusqlite::Result<BusinessTransaction<'_>> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let savepoint = if connection.is_autocommit() {
        None
    } else {
        Some(format!(
            "tron_tx_{}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    };
    if let Some(name) = &savepoint {
        connection.execute_batch(&format!("SAVEPOINT {name}"))?;
    } else {
        connection.execute_batch(match behavior {
            rusqlite::TransactionBehavior::Deferred => "BEGIN DEFERRED",
            rusqlite::TransactionBehavior::Exclusive => "BEGIN EXCLUSIVE",
            _ => "BEGIN IMMEDIATE",
        })?;
    }
    Ok(BusinessTransaction {
        connection,
        savepoint,
        finished: false,
    })
}
impl std::ops::Deref for BusinessTransaction<'_> {
    type Target = rusqlite::Connection;
    fn deref(&self) -> &Self::Target {
        self.connection
    }
}
impl BusinessTransaction<'_> {
    pub fn commit(mut self) -> rusqlite::Result<()> {
        self.connection.execute_batch(
            &self
                .savepoint
                .as_ref()
                .map(|name| format!("RELEASE {name}"))
                .unwrap_or("COMMIT".into()),
        )?;
        self.finished = true;
        Ok(())
    }
    pub fn rollback(mut self) -> rusqlite::Result<()> {
        self.rollback_inner()?;
        self.finished = true;
        Ok(())
    }
    fn rollback_inner(&self) -> rusqlite::Result<()> {
        self.connection.execute_batch(
            &self
                .savepoint
                .as_ref()
                .map(|name| format!("ROLLBACK TO {name}; RELEASE {name}"))
                .unwrap_or("ROLLBACK".into()),
        )
    }
}
impl Drop for BusinessTransaction<'_> {
    fn drop(&mut self) {
        if !self.finished {
            if let Err(error) = self.rollback_inner() {
                tracing::error!("business transaction rollback failed: {error}");
            }
        }
    }
}
