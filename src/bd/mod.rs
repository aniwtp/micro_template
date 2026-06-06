use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use shodh_redb::ttl_table::TtlTableDefinition;
use shodh_redb::{Database, Key, TableDefinition, TableHandle, Value};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Redb(#[from] shodh_redb::error::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("no write buffer registered for this table")]
    NoBuffer,
}

macro_rules! db {
    ($e:expr) => {
        $e.map_err(|e| DbError::Redb(e.into()))
    };
}

// ---------------------------------------------------------------------------
// RawBuffer — type-erased buffer trait
// ---------------------------------------------------------------------------

/// Type-erased interface for a write buffer.  Implemented by
/// [`WriteBuffer<K, V>`] so that `DBWrapper` can store buffers of
/// different `K, V` types in a single collection.
trait RawBuffer: Send + Sync + 'static {
    /// Push pre-serialised key and value bytes.
    fn push_raw(&self, k_bytes: Vec<u8>, v_bytes: Vec<u8>) -> Result<(), DbError>;
    /// Flush all pending entries.
    fn flush(&self) -> Result<usize, DbError>;
    fn len(&self) -> usize;
    /// Whether every `push` should immediately flush (TTL safety).
    fn force_flush(&self) -> bool;
}

// ---------------------------------------------------------------------------
// DBWrapper — database handle with backup + compaction + buffers
// ---------------------------------------------------------------------------
pub struct DBWrapper {
    db: Arc<Database>,
    buffers: Arc<Mutex<HashMap<String, Arc<dyn RawBuffer>>>>,
}

impl Clone for DBWrapper {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            buffers: self.buffers.clone(),
        }
    }
}

// Manual Debug — `dyn RawBuffer` doesn't implement Debug.
impl std::fmt::Debug for DBWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DBWrapper")
            .field("db", &self.db)
            .field("buffers", &self.buffers.lock().unwrap().len())
            .finish()
    }
}

impl DBWrapper {
    /// Open (or create) the redb database at `path`.
    pub fn new(path: &str) -> Result<Self, DbError> {
        let db = Arc::new(db!(Database::create(path))?);
        Ok(Self {
            db,
            buffers: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    // -- Maintenance --------------------------------------------------------

    /// Run a full compaction.
    pub fn compact(&self) -> Result<(), DbError> {
        let handle = db!(self.db.start_compaction())?;
        let steps = db!(handle.run())?;
        log::info!("database compaction completed ({steps} steps)");
        Ok(())
    }

    /// Create a timestamped backup inside the `backups/` directory.
    pub fn backup(&self) -> Result<(), DbError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let backup_dir = "backups";
        std::fs::create_dir_all(backup_dir)?;

        let backup_path = format!("{backup_dir}/redb_{now}.redb");
        db!(self.db.backup(&backup_path))?;

        log::info!("backup saved to {backup_path}");
        Ok(())
    }

    // -- Async maintenance loop (compio via ntex) ---------------------------

    /// Spawn a background task that compacts and backs up the database once
    /// every 24 hours, using the **compio** async runtime (via `ntex`).
    pub fn spawn_maintenance_loop(&self) {
        let db = self.db.clone();
        drop(ntex::rt::spawn(async move {
            let day = Duration::from_secs(24 * 60 * 60);
            loop {
                ntex::time::sleep(day).await;

                // compact
                let compact_res = (|| -> Result<(), DbError> {
                    let handle = db!(db.start_compaction())?;
                    let steps = db!(handle.run())?;
                    log::info!("database compaction completed ({steps} steps)");
                    Ok(())
                })();
                if let Err(e) = compact_res {
                    log::error!("compaction error: {e}");
                }

                // backup
                let backup_res = (|| -> Result<(), DbError> {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let backup_dir = "backups";
                    std::fs::create_dir_all(backup_dir)?;
                    let backup_path = format!("{backup_dir}/redb_{now}.redb");
                    db!(db.backup(&backup_path))?;
                    log::info!("backup saved to {backup_path}");
                    Ok(())
                })();
                if let Err(e) = backup_res {
                    log::error!("backup error: {e}");
                }
            }
        }));
    }

    // -- Buffer creation ----------------------------------------------------

    /// Register a write buffer for a regular table.
    ///
    /// After creation, push entries via [`push_to`](Self::push_to) and
    /// flush via [`flush_table`](Self::flush_table).
    pub fn create_write_buffer<K, V>(
        &self,
        table_def: TableDefinition<'static, K, V>,
        max_buffer_size: usize,
        flush_interval: Duration,
    ) where
        K: Key + Send + Sync + 'static,
        V: Value + Send + Sync + 'static,
    {
        let wb = WriteBuffer::new(
            self.db.clone(),
            TableKind::Regular(table_def),
            max_buffer_size,
            flush_interval,
        );
        let mut buffers = self.buffers.lock().unwrap();
        buffers.insert(table_def.name().to_owned(), Arc::new(wb));
    }

    /// Register a write buffer for a TTL-enabled table.
    ///
    /// Every entry will use the given `ttl`.  If `flush_interval` is
    /// **longer** than `ttl`, each [`push_to`](Self::push_to) immediately
    /// flushes to avoid holding data past its expiry.
    pub fn create_ttl_write_buffer<K, V>(
        &self,
        ttl_def: TtlTableDefinition<K, V>,
        max_buffer_size: usize,
        flush_interval: Duration,
        ttl: Duration,
    ) where
        K: Key + Send + Sync + 'static,
        V: Value + Send + Sync + 'static,
    {
        let wb = WriteBuffer::new(
            self.db.clone(),
            TableKind::Ttl { def: ttl_def, ttl },
            max_buffer_size,
            flush_interval,
        );
        let mut buffers = self.buffers.lock().unwrap();
        buffers.insert(ttl_def.name().to_owned(), Arc::new(wb));
    }

    // -- Buffer operations --------------------------------------------------

    /// Push a key-value pair into the registered buffer, **or** write
    /// directly to the database if no buffer exists for this table.
    ///
    /// This is a convenience that lets you call the same method
    /// regardless of whether you set up a write buffer at init time.
    pub fn write_or_buffer<K, V>(
        &self,
        table_def: TableDefinition<'static, K, V>,
        key: K,
        value: V,
    ) -> Result<(), DbError>
    where
        K: Key + Send + 'static,
        V: Value + Send + 'static,
    {
        let buffers = self.buffers.lock().unwrap();
        if let Some(buf) = buffers.get(table_def.name()) {
            let buf = buf.clone();
            drop(buffers);

            if buf.force_flush() {
                buf.flush()?;
            }
            let k_bytes = serialize_value(&key);
            let v_bytes = serialize_value(&value);
            return buf.push_raw(k_bytes, v_bytes);
        }
        drop(buffers);

        // No buffer — write directly.
        let k_bytes = serialize_value(&key);
        let v_bytes = serialize_value(&value);
        let write_txn = db!(self.db.begin_write())?;
        {
            let mut table = db!(write_txn.open_table(table_def))?;
            let k = K::from_bytes(&k_bytes);
            let v = V::from_bytes(&v_bytes);
            db!(table.insert(k, v))?;
        }
        db!(write_txn.commit())?;
        Ok(())
    }

    /// Same as [`write_or_buffer`] but for TTL tables.
    pub fn write_ttl_or_buffer<K, V>(
        &self,
        ttl_def: TtlTableDefinition<K, V>,
        key: K,
        value: V,
        ttl: Duration,
    ) -> Result<(), DbError>
    where
        K: Key + Send + 'static,
        V: Value + Send + 'static,
    {
        let buffers = self.buffers.lock().unwrap();
        if let Some(buf) = buffers.get(ttl_def.name()) {
            let buf = buf.clone();
            drop(buffers);

            if buf.force_flush() {
                buf.flush()?;
            }
            let k_bytes = serialize_value(&key);
            let v_bytes = serialize_value(&value);
            return buf.push_raw(k_bytes, v_bytes);
        }
        drop(buffers);

        // No buffer — write directly with TTL.
        let k_bytes = serialize_value(&key);
        let v_bytes = serialize_value(&value);
        let write_txn = db!(self.db.begin_write())?;
        {
            let mut table = db!(write_txn.open_ttl_table(ttl_def))?;
            let k = K::from_bytes(&k_bytes);
            let v = V::from_bytes(&v_bytes);
            db!(table.insert_with_ttl(k, v, ttl))?;
        }
        db!(write_txn.commit())?;
        Ok(())
    }

    /// Flush all pending entries for the given table.
    pub fn flush_table(&self, table_name: &str) -> Result<usize, DbError> {
        let buffers = self.buffers.lock().unwrap();
        let buf = buffers.get(table_name).ok_or(DbError::NoBuffer)?.clone();
        drop(buffers);
        buf.flush()
    }

    /// Flush all registered buffers.
    pub fn flush_all(&self) -> Result<(), DbError> {
        let buffers = self.buffers.lock().unwrap();
        for buf in buffers.values() {
            buf.flush()?;
        }
        Ok(())
    }

    /// Number of pending entries in a specific buffer.
    pub fn buffer_len(&self, table_name: &str) -> Result<usize, DbError> {
        let buffers = self.buffers.lock().unwrap();
        let buf = buffers.get(table_name).ok_or(DbError::NoBuffer)?;
        Ok(buf.len())
    }
}

// ---------------------------------------------------------------------------
// TableKind — discriminates regular vs TTL tables
// ---------------------------------------------------------------------------

enum TableKind<K: Key + 'static, V: Value + 'static> {
    Regular(TableDefinition<'static, K, V>),
    Ttl {
        def: TtlTableDefinition<K, V>,
        ttl: Duration,
    },
}

// ---------------------------------------------------------------------------
// Internal unsafe helper
// ---------------------------------------------------------------------------

/// Reinterprets a `&T` as `&T::SelfType<'_>`.
///
/// # Safety
///
/// For every type implementing [`Value`] in `shodh_redb`, the associated
/// type `SelfType<'a>` is either:
/// * `T` itself (for all owned types such as `u64`, `String`, …), or
/// * a reference type with identical layout (for `&str`, `&[u8]`, …).
///
/// In both cases transmuting `&T` → `&T::SelfType<'_>` is sound.
unsafe fn as_self_type_ref<T: Value>(value: &T) -> &T::SelfType<'_> {
    unsafe { &*(value as *const T as *const T::SelfType<'_>) }
}

/// Serialise a `Value` to bytes.
fn serialize_value<T: Value>(value: &T) -> Vec<u8> {
    // SAFETY: `as_self_type_ref` is valid for all `shodh_redb::Value` types.
    T::as_bytes(unsafe { as_self_type_ref(value) })
        .as_ref()
        .to_vec()
}

// ---------------------------------------------------------------------------
// WriteBuffer — batch-insert buffer with auto-flush
// ---------------------------------------------------------------------------

/// A write buffer that batches inserts for a single redb table
/// (regular or TTL).
///
/// Two triggers cause an automatic flush:
/// 1. **Size** — the number of pending entries reaches `max_size`.
/// 2. **Time** — `flush_interval` elapses since the last flush.
///
/// For TTL buffers: if `flush_interval > ttl`, every push immediately
/// flushes so that entries are never held in memory past their expiry.
struct WriteBuffer<K, V>
where
    K: Key + Send + 'static,
    V: Value + Send + 'static,
{
    inner: Arc<WriteBufferInner<K, V>>,
}

struct WriteBufferInner<K, V>
where
    K: Key + Send + 'static,
    V: Value + Send + 'static,
{
    db: Arc<Database>,
    kind: TableKind<K, V>,
    /// Serialised entries: (key_bytes, value_bytes).
    buffer: Mutex<Vec<(Vec<u8>, Vec<u8>)>>,
    max_size: usize,
    flush_interval: Duration,
    last_flush: Mutex<Instant>,
    alive: AtomicBool,
}

impl<K, V> WriteBuffer<K, V>
where
    K: Key + Send + 'static,
    V: Value + Send + 'static,
{
    fn new(
        db: Arc<Database>,
        kind: TableKind<K, V>,
        max_size: usize,
        flush_interval: Duration,
    ) -> Self {
        let inner = Arc::new(WriteBufferInner {
            db,
            kind,
            buffer: Mutex::new(Vec::with_capacity(max_size)),
            max_size,
            flush_interval,
            last_flush: Mutex::new(Instant::now()),
            alive: AtomicBool::new(true),
        });

        // Spawn a background task that flushes on the timer.
        let bg = Arc::downgrade(&inner);
        drop(ntex::rt::spawn(async move {
            ntex::time::sleep(flush_interval).await;
            loop {
                let Some(inner) = bg.upgrade() else {
                    return;
                };
                if !inner.alive.load(Ordering::Acquire) {
                    return;
                }

                let elapsed = inner.last_flush.lock().unwrap().elapsed();
                if elapsed >= inner.flush_interval
                    && let Err(e) = Self::flush_inner(&inner)
                {
                    log::error!("write-buffer auto-flush error: {e}");
                }
                ntex::time::sleep(inner.flush_interval).await;
            }
        }));

        Self { inner }
    }

    fn flush_inner(inner: &WriteBufferInner<K, V>) -> Result<usize, DbError> {
        let mut buf = inner.buffer.lock().unwrap();
        if buf.is_empty() {
            return Ok(0);
        }
        let entries: Vec<_> = std::mem::take(&mut *buf);
        drop(buf);

        let count = entries.len();

        let write_txn = db!(inner.db.begin_write())?;
        match &inner.kind {
            TableKind::Regular(table_def) => {
                let mut table = db!(write_txn.open_table(*table_def))?;
                for (k_bytes, v_bytes) in &entries {
                    let key = K::from_bytes(k_bytes);
                    let value = V::from_bytes(v_bytes);
                    db!(table.insert(key, value))?;
                }
            }
            TableKind::Ttl { def, ttl } => {
                let mut table = db!(write_txn.open_ttl_table(*def))?;
                for (k_bytes, v_bytes) in &entries {
                    let key = K::from_bytes(k_bytes);
                    let value = V::from_bytes(v_bytes);
                    db!(table.insert_with_ttl(key, value, *ttl))?;
                }
            }
        }
        db!(write_txn.commit())?;

        *inner.last_flush.lock().unwrap() = Instant::now();
        Ok(count)
    }
}

// ---------------------------------------------------------------------------
// RawBuffer impl for WriteBuffer
// ---------------------------------------------------------------------------

impl<K, V> RawBuffer for WriteBuffer<K, V>
where
    K: Key + Send + Sync + 'static,
    V: Value + Send + Sync + 'static,
{
    fn push_raw(&self, k_bytes: Vec<u8>, v_bytes: Vec<u8>) -> Result<(), DbError> {
        let mut buf = self.inner.buffer.lock().unwrap();
        buf.push((k_bytes, v_bytes));
        let should_flush = buf.len() >= self.inner.max_size;
        drop(buf);

        if should_flush {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&self) -> Result<usize, DbError> {
        Self::flush_inner(&self.inner)
    }

    fn len(&self) -> usize {
        self.inner.buffer.lock().unwrap().len()
    }

    fn force_flush(&self) -> bool {
        match &self.inner.kind {
            TableKind::Ttl { ttl, .. } => self.inner.flush_interval > *ttl,
            TableKind::Regular(_) => false,
        }
    }
}

impl<K, V> Drop for WriteBuffer<K, V>
where
    K: Key + Send + 'static,
    V: Value + Send + 'static,
{
    fn drop(&mut self) {
        self.inner.alive.store(false, Ordering::Release);
        if let Err(e) = Self::flush_inner(&self.inner) {
            log::error!("write-buffer drop-flush error: {e}");
        }
    }
}
