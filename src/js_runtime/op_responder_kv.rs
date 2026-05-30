//! Per-responder key-value store exposed to responder scripts under `secutils.kv.*` (`get` /
//! `set` / `delete` / `list` / `watch`).
//!
//! Every operation is implicitly scoped to the responder that is currently executing - a script can
//! never read or write another responder's KV. The backing store is the
//! `user_data_webhooks_responders_kv` table, rows optionally carry an `expires_at` for TTL
//! semantics.
//!
//! ## Runtime model
//!
//! Scripts run on a dedicated pool of worker threads, each owning its own `CurrentThread` tokio
//! runtime (see [`crate::js_runtime::worker_pool`]). The shared `sqlx` connection pool, however,
//! lives on the server's request runtime. To avoid polling a connection from a runtime other than
//! the one it was established on, every database round-trip is `spawn`ed onto the request runtime
//! via the [`tokio::runtime::Handle`] captured when [`KvState`] is built, the op merely awaits the
//! resulting `JoinHandle`, which is runtime-agnostic.
//!
//! Binary values cross the JS boundary as base64url strings so the ops can stay on the simple
//! `#[string]` / `#[serde]` op2 marshalling path.

use base64ct::{Base64UrlUnpadded, Encoding};
use dashmap::DashMap;
use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use time::OffsetDateTime;
use tokio::{runtime::Handle, sync::Notify};
use uuid::Uuid;

/// Default `watch` long-poll budget when the caller does not specify `timeoutMs`.
const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_millis(25_000);
/// Periodic re-list cadence inside `watch`. Bounds the staleness window when a notification is
/// missed (e.g. the writer and waiter are on different replicas and the in-process notifier never
/// fires for this node).
const WAIT_FALLBACK_INTERVAL: Duration = Duration::from_millis(2_000);
/// Hard ceiling for any single `list`/`watch` page, regardless of caller input.
const MAX_LIST_LIMIT: i64 = 1_000;

/// Per-tier quotas applied to KV writes. Copied out of the responder owner's subscription
/// configuration when [`KvState`] is constructed.
#[derive(Clone, Copy, Debug)]
pub struct KvQuotas {
    pub max_key_bytes: usize,
    pub max_value_bytes: usize,
    pub max_entries: usize,
    pub max_total_bytes: usize,
    pub max_ttl_sec: u64,
    /// Absolute ceiling on any row's lifetime: every write expires no later than `now + this`, and
    /// TTL-less writes are forced to expire at that bound. `0` disables the cap.
    pub max_lifespan_sec: u64,
}

/// In-process pub/sub used by [`op_responder_kv_wait`]. A single [`Notify`] per responder is woken
/// on every committed `set`, waiters always perform an authoritative `list` before and after
/// sleeping, so correctness never depends on a notification actually being delivered (the periodic
/// fallback re-list upgrades a missed notify into bounded extra latency rather than a lost update).
/// This makes the design correct across replicas without Postgres `LISTEN`/`NOTIFY`.
#[derive(Clone, Default)]
pub struct WebhooksKvNotifier {
    inner: Arc<DashMap<Uuid, Arc<Notify>>>,
}

impl WebhooksKvNotifier {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    /// Returns (creating if necessary) the notify handle for a responder.
    fn handle(&self, responder_id: Uuid) -> Arc<Notify> {
        self.inner
            .entry(responder_id)
            .or_insert_with(|| Arc::new(Notify::new()))
            .clone()
    }

    /// Wakes every waiter currently parked on the responder's notify handle.
    pub fn notify(&self, responder_id: Uuid) {
        if let Some(notify) = self.inner.get(&responder_id) {
            notify.notify_waiters();
        }
    }
}

/// State injected into Deno's `OpState` for a single responder script execution
/// that is allowed to use the KV primitive.
pub struct KvState {
    pub pool: sqlx::Pool<sqlx::Postgres>,
    pub handle: Handle,
    pub responder_id: Uuid,
    pub notifier: WebhooksKvNotifier,
    pub quotas: KvQuotas,
    /// Remaining KV operations for this script invocation.
    pub ops_remaining: AtomicUsize,
    /// Wall-clock instant past which `watch` must stop blocking and return
    /// `timedOut: true` (kept comfortably inside the script's hard time limit).
    pub deadline: Instant,
}

impl KvState {
    /// Builds the state, deriving the `watch` deadline from the script's wall
    /// clock budget so a long-poll always yields gracefully before the runtime
    /// force-terminates the isolate.
    pub fn new(
        pool: sqlx::Pool<sqlx::Postgres>,
        handle: Handle,
        responder_id: Uuid,
        notifier: WebhooksKvNotifier,
        quotas: KvQuotas,
        ops_budget: usize,
        script_budget: Duration,
    ) -> Self {
        let safety_margin = Duration::from_millis(1_000);
        let effective = script_budget.saturating_sub(safety_margin);
        Self {
            pool,
            handle,
            responder_id,
            notifier,
            quotas,
            ops_remaining: AtomicUsize::new(ops_budget),
            deadline: Instant::now() + effective,
        }
    }

    /// Decrements the op budget, failing once it is exhausted.
    fn consume_op(&self) -> Result<(), JsErrorBox> {
        loop {
            let current = self.ops_remaining.load(Ordering::Relaxed);
            if current == 0 {
                return Err(JsErrorBox::generic(
                    "Responder KV operation budget exhausted.".to_string(),
                ));
            }
            if self
                .ops_remaining
                .compare_exchange_weak(current, current - 1, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return Ok(());
            }
        }
    }
}

/// Snapshot of the per-op state needed to talk to the database off-thread.
struct KvHandle {
    pool: sqlx::Pool<sqlx::Postgres>,
    runtime: Handle,
    responder_id: Uuid,
    notifier: WebhooksKvNotifier,
    quotas: KvQuotas,
    deadline: Instant,
}

/// Borrows [`KvState`] from the op state, consumes one unit of op budget, and returns an owned
/// [`KvHandle`] usable across `await` points.
fn checkout(state: &Rc<RefCell<OpState>>) -> Result<KvHandle, JsErrorBox> {
    let state = state.borrow();
    let kv = state.try_borrow::<KvState>().ok_or_else(|| {
        JsErrorBox::generic("KV storage is not available for this responder.".to_string())
    })?;
    kv.consume_op()?;
    Ok(KvHandle {
        pool: kv.pool.clone(),
        runtime: kv.handle.clone(),
        responder_id: kv.responder_id,
        notifier: kv.notifier.clone(),
        quotas: kv.quotas,
        deadline: kv.deadline,
    })
}

fn db_err(err: sqlx::Error) -> JsErrorBox {
    JsErrorBox::generic(format!("KV database error: {err}"))
}

fn join_err<E: std::fmt::Display>(err: E) -> JsErrorBox {
    JsErrorBox::generic(format!("KV task failed: {err}"))
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SetOpts {
    /// Time-to-live in seconds. Clamped to the tier's `responder_kv_max_ttl_sec`.
    pub ttl_sec: Option<u64>,
    /// When `true`, an existing *live* row for the key is preserved (first-writer-wins), expired
    /// rows are always reclaimable.
    #[serde(default)]
    pub if_absent: bool,
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListOpts {
    pub prefix: Option<String>,
    pub after: Option<String>,
    pub limit: Option<i64>,
    #[serde(default)]
    pub values_included: Option<bool>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WaitOpts {
    pub prefix: String,
    pub after: Option<String>,
    pub limit: Option<i64>,
    pub timeout_ms: Option<u64>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KvEntry {
    pub key: String,
    /// Row creation time as a unix timestamp (seconds).
    pub created_at: i64,
    /// Base64url of the stored value, or `null` when values were not requested.
    pub value: Option<String>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GetResult {
    /// Base64url of the stored value, or `null` when the key is absent/expired.
    pub value: Option<String>,
    /// Unix-seconds expiry of the row, or `null` when the key is absent or never expires. Lets a
    /// script anchor dependent writes (e.g. captured requests) to the row's deadline.
    pub expires_at: Option<i64>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ListResult {
    pub entries: Vec<KvEntry>,
    /// Full key of the last returned entry (an opaque pagination cursor), if any.
    pub cursor: Option<String>,
    /// `true` when a `watch` returned because its deadline elapsed with no rows.
    pub timed_out: bool,
}

fn clamp_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(MAX_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT)
}

/// Reads a single live value (expired rows are treated as absent), independent of the Deno op
/// machinery so it can be exercised directly against a pool in tests. The production read path uses
/// [`get_entry`] (which also surfaces the row's expiry), this value-only helper is retained for the
/// store's unit tests.
#[cfg(test)]
async fn get_value(
    pool: &sqlx::Pool<sqlx::Postgres>,
    responder_id: Uuid,
    key: &str,
) -> Result<Option<Vec<u8>>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
SELECT value FROM user_data_webhooks_responders_kv
WHERE responder_id = $1 AND key = $2 AND (expires_at IS NULL OR expires_at > now())
        "#,
        responder_id,
        key
    )
    .fetch_optional(pool)
    .await
}

/// Reads a single live row's value together with its expiry, treating expired rows as absent.
/// Pool-level counterpart of [`op_responder_kv_get`], kept free of op state for direct testing.
async fn get_entry(
    pool: &sqlx::Pool<sqlx::Postgres>,
    responder_id: Uuid,
    key: &str,
) -> Result<Option<(Vec<u8>, Option<OffsetDateTime>)>, sqlx::Error> {
    Ok(sqlx::query!(
        r#"
SELECT value, expires_at FROM user_data_webhooks_responders_kv
WHERE responder_id = $1 AND key = $2 AND (expires_at IS NULL OR expires_at > now())
        "#,
        responder_id,
        key
    )
    .fetch_optional(pool)
    .await?
    .map(|row| (row.value, row.expires_at)))
}

/// Deletes a key, returning the number of rows removed. Pool-level counterpart of
/// [`op_responder_kv_delete`], kept free of op state for direct testing.
async fn delete_value(
    pool: &sqlx::Pool<sqlx::Postgres>,
    responder_id: Uuid,
    key: &str,
) -> Result<u64, sqlx::Error> {
    sqlx::query!(
        r#"DELETE FROM user_data_webhooks_responders_kv WHERE responder_id = $1 AND key = $2"#,
        responder_id,
        key
    )
    .execute(pool)
    .await
    .map(|result| result.rows_affected())
}

/// Quota-checked write shared by [`op_responder_kv_set`] and the unit tests. The outer `Result`
/// carries genuine database failures; the inner `Result` carries a human-readable quota-violation
/// message (`Err`) versus a committed write (`Ok`). Keeping all size/entry/byte enforcement here -
/// rather than split between the op and the DB closure - means a single function fully describes
/// (and can be tested for) the write contract.
#[allow(clippy::too_many_arguments)]
async fn set_value(
    pool: &sqlx::Pool<sqlx::Postgres>,
    responder_id: Uuid,
    key: &str,
    value: &[u8],
    expires_at: Option<OffsetDateTime>,
    if_absent: bool,
    quotas: KvQuotas,
) -> Result<Result<(), String>, sqlx::Error> {
    if key.len() > quotas.max_key_bytes {
        return Ok(Err(format!(
            "KV key exceeds the maximum of {} bytes.",
            quotas.max_key_bytes
        )));
    }
    if value.len() > quotas.max_value_bytes {
        return Ok(Err(format!(
            "KV value exceeds the maximum of {} bytes.",
            quotas.max_value_bytes
        )));
    }

    // Existing live size for this key (to compute the byte-quota delta on upsert).
    let existing_len: Option<i32> = sqlx::query_scalar!(
        r#"
SELECT length(value) AS "len!" FROM user_data_webhooks_responders_kv
WHERE responder_id = $1 AND key = $2 AND (expires_at IS NULL OR expires_at > now())
        "#,
        responder_id,
        key
    )
    .fetch_optional(pool)
    .await?;

    let aggregate = sqlx::query!(
        r#"
SELECT COUNT(*) AS "count!", COALESCE(SUM(length(value)), 0)::bigint AS "total!"
FROM user_data_webhooks_responders_kv
WHERE responder_id = $1 AND (expires_at IS NULL OR expires_at > now())
        "#,
        responder_id
    )
    .fetch_one(pool)
    .await?;

    let live_count = aggregate.count as usize;
    let live_total = aggregate.total as usize;
    let projected_count = live_count + usize::from(existing_len.is_none());
    let projected_total = live_total - existing_len.unwrap_or(0) as usize + value.len();

    if projected_count > quotas.max_entries {
        return Ok(Err(format!(
            "KV entry limit reached ({} entries).",
            quotas.max_entries
        )));
    }
    if projected_total > quotas.max_total_bytes {
        return Ok(Err(format!(
            "KV total size limit reached ({} bytes).",
            quotas.max_total_bytes
        )));
    }

    if if_absent {
        sqlx::query!(
            r#"
INSERT INTO user_data_webhooks_responders_kv (responder_id, key, value, created_at, expires_at)
VALUES ($1, $2, $3, now(), $4)
ON CONFLICT (responder_id, key) DO UPDATE
SET value = EXCLUDED.value, created_at = EXCLUDED.created_at, expires_at = EXCLUDED.expires_at
WHERE user_data_webhooks_responders_kv.expires_at IS NOT NULL
  AND user_data_webhooks_responders_kv.expires_at <= now()
            "#,
            responder_id,
            key,
            value,
            expires_at
        )
        .execute(pool)
        .await?;
    } else {
        sqlx::query!(
            r#"
INSERT INTO user_data_webhooks_responders_kv (responder_id, key, value, created_at, expires_at)
VALUES ($1, $2, $3, now(), $4)
ON CONFLICT (responder_id, key) DO UPDATE
SET value = EXCLUDED.value, created_at = EXCLUDED.created_at, expires_at = EXCLUDED.expires_at
            "#,
            responder_id,
            key,
            value,
            expires_at
        )
        .execute(pool)
        .await?;
    }

    Ok(Ok(()))
}

/// Reads a single value, transparently treating expired rows as absent.
#[op2]
#[serde]
pub async fn op_responder_kv_get(
    state: Rc<RefCell<OpState>>,
    #[string] key: String,
) -> Result<GetResult, JsErrorBox> {
    let kv = checkout(&state)?;
    let pool = kv.pool;
    let responder_id = kv.responder_id;
    let entry = kv
        .runtime
        .spawn(async move { get_entry(&pool, responder_id, &key).await })
        .await
        .map_err(join_err)?
        .map_err(db_err)?;

    Ok(match entry {
        Some((bytes, expires_at)) => GetResult {
            value: Some(Base64UrlUnpadded::encode_string(&bytes)),
            expires_at: expires_at.map(|at| at.unix_timestamp()),
        },
        None => GetResult {
            value: None,
            expires_at: None,
        },
    })
}

/// Resolves the absolute expiry for a write from the requested TTL and the tier quotas.
///
/// Two independent caps apply:
/// - `max_ttl_sec` bounds an explicitly requested TTL.
/// - `max_lifespan_sec` is an absolute ceiling: when set (> 0) it caps the expiry at
///   `now + max_lifespan_sec` **and** forces a finite expiry on TTL-less writes, so nothing the
///   store holds can outlive the bound (the self-cleaning backstop). `0` leaves TTL-less writes
///   eternal.
fn effective_expires_at(
    now: OffsetDateTime,
    ttl_sec: Option<u64>,
    quotas: KvQuotas,
) -> Option<OffsetDateTime> {
    let requested = ttl_sec.map(|ttl_sec| {
        let ttl_sec = if quotas.max_ttl_sec > 0 {
            ttl_sec.min(quotas.max_ttl_sec)
        } else {
            ttl_sec
        };
        now + Duration::from_secs(ttl_sec)
    });
    let lifespan_cap =
        (quotas.max_lifespan_sec > 0).then(|| now + Duration::from_secs(quotas.max_lifespan_sec));
    match (requested, lifespan_cap) {
        (Some(req), Some(cap)) => Some(req.min(cap)),
        (Some(req), None) => Some(req),
        (None, cap) => cap,
    }
}

/// Writes a value (base64url) with optional TTL and first-writer-wins semantics, enforcing the
/// responder's per-tier entry/byte quotas, then notifies waiters.
#[op2]
pub async fn op_responder_kv_set(
    state: Rc<RefCell<OpState>>,
    #[string] key: String,
    #[string] value_b64: String,
    #[serde] opts: SetOpts,
) -> Result<(), JsErrorBox> {
    let kv = checkout(&state)?;
    let quotas = kv.quotas;

    let value = Base64UrlUnpadded::decode_vec(&value_b64)
        .map_err(|err| JsErrorBox::generic(format!("KV value is not valid base64url: {err}")))?;

    let expires_at = effective_expires_at(OffsetDateTime::now_utc(), opts.ttl_sec, quotas);

    let pool = kv.pool;
    let responder_id = kv.responder_id;
    let if_absent = opts.if_absent;

    kv.runtime
        .spawn(async move {
            set_value(
                &pool,
                responder_id,
                &key,
                &value,
                expires_at,
                if_absent,
                quotas,
            )
            .await
        })
        .await
        .map_err(join_err)?
        .map_err(db_err)?
        .map_err(JsErrorBox::generic)?;

    kv.notifier.notify(responder_id);
    Ok(())
}

/// Deletes a key, returning whether a row was removed.
#[op2]
pub async fn op_responder_kv_delete(
    state: Rc<RefCell<OpState>>,
    #[string] key: String,
) -> Result<bool, JsErrorBox> {
    let kv = checkout(&state)?;
    let pool = kv.pool;
    let responder_id = kv.responder_id;
    let affected = kv
        .runtime
        .spawn(async move { delete_value(&pool, responder_id, &key).await })
        .await
        .map_err(join_err)?
        .map_err(db_err)?;

    Ok(affected > 0)
}

/// Performs the authoritative, expiry-aware prefix scan shared by `list`/`watch`.
async fn list_entries(
    pool: &sqlx::Pool<sqlx::Postgres>,
    responder_id: Uuid,
    prefix: Option<String>,
    after: Option<String>,
    limit: i64,
    values_included: bool,
) -> Result<Vec<KvEntry>, sqlx::Error> {
    if values_included {
        let rows = sqlx::query!(
            r#"
SELECT key, created_at, value FROM user_data_webhooks_responders_kv
WHERE responder_id = $1
  AND ($2::text IS NULL OR starts_with(key, $2))
  AND ($3::text IS NULL OR key > $3)
  AND (expires_at IS NULL OR expires_at > now())
ORDER BY key ASC
LIMIT $4
            "#,
            responder_id,
            prefix,
            after,
            limit
        )
        .fetch_all(pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| KvEntry {
                key: row.key,
                created_at: row.created_at.unix_timestamp(),
                value: Some(Base64UrlUnpadded::encode_string(&row.value)),
            })
            .collect())
    } else {
        let rows = sqlx::query!(
            r#"
SELECT key, created_at FROM user_data_webhooks_responders_kv
WHERE responder_id = $1
  AND ($2::text IS NULL OR starts_with(key, $2))
  AND ($3::text IS NULL OR key > $3)
  AND (expires_at IS NULL OR expires_at > now())
ORDER BY key ASC
LIMIT $4
            "#,
            responder_id,
            prefix,
            after,
            limit
        )
        .fetch_all(pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| KvEntry {
                key: row.key,
                created_at: row.created_at.unix_timestamp(),
                value: None,
            })
            .collect())
    }
}

/// Lists keys (optionally with values) under a prefix, paginated by `after`.
#[op2]
#[serde]
pub async fn op_responder_kv_list(
    state: Rc<RefCell<OpState>>,
    #[serde] opts: ListOpts,
) -> Result<ListResult, JsErrorBox> {
    let kv = checkout(&state)?;
    let pool = kv.pool;
    let responder_id = kv.responder_id;
    let limit = clamp_limit(opts.limit);
    let values_included = opts.values_included.unwrap_or(true);
    let prefix = opts.prefix;
    let after = opts.after;

    let entries = kv
        .runtime
        .spawn(async move {
            list_entries(&pool, responder_id, prefix, after, limit, values_included).await
        })
        .await
        .map_err(join_err)?
        .map_err(db_err)?;

    let cursor = entries.last().map(|entry| entry.key.clone());
    Ok(ListResult {
        entries,
        cursor,
        timed_out: false,
    })
}

/// Returns matching rows immediately if any exist after the cursor, otherwise long-polls until a
/// matching `set` lands or the deadline elapses. The initial (and every fallback) `list` is the
/// source of truth, so a missed in-process notification only costs up to [`WAIT_FALLBACK_INTERVAL`]
/// of latency.
#[op2]
#[serde]
pub async fn op_responder_kv_wait(
    state: Rc<RefCell<OpState>>,
    #[serde] opts: WaitOpts,
) -> Result<ListResult, JsErrorBox> {
    let kv = checkout(&state)?;
    let pool = kv.pool;
    let responder_id = kv.responder_id;
    let limit = clamp_limit(opts.limit);
    let prefix = opts.prefix;
    let after = opts.after;

    let requested = opts
        .timeout_ms
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_WAIT_TIMEOUT);
    let deadline = std::cmp::min(Instant::now() + requested, kv.deadline);

    // Register interest before the first list so a notification that races our sleep is not lost
    // (the future only enrolls once enabled/awaited).
    let notify = kv.notifier.handle(responder_id);
    let mut notified = Box::pin(notify.notified());
    notified.as_mut().enable();

    loop {
        let scan_pool = pool.clone();
        let scan_prefix = Some(prefix.clone());
        let scan_after = after.clone();
        let entries = kv
            .runtime
            .spawn(async move {
                list_entries(
                    &scan_pool,
                    responder_id,
                    scan_prefix,
                    scan_after,
                    limit,
                    true,
                )
                .await
            })
            .await
            .map_err(join_err)?
            .map_err(db_err)?;

        if !entries.is_empty() {
            let cursor = entries.last().map(|entry| entry.key.clone());
            return Ok(ListResult {
                entries,
                cursor,
                timed_out: false,
            });
        }

        let now = Instant::now();
        if now >= deadline {
            return Ok(ListResult {
                entries: Vec::new(),
                cursor: None,
                timed_out: true,
            });
        }

        let until_deadline = deadline - now;
        let sleep_for = std::cmp::min(until_deadline, WAIT_FALLBACK_INTERVAL);
        tokio::select! {
            _ = &mut notified => {
                notified = Box::pin(notify.notified());
                notified.as_mut().enable();
            }
            _ = tokio::time::sleep(sleep_for) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        KvQuotas, ListOpts, SetOpts, WaitOpts, delete_value, effective_expires_at, get_entry,
        get_value, list_entries, set_value,
    };
    use sqlx::PgPool;
    use time::{Duration as TimeDuration, OffsetDateTime};
    use uuid::Uuid;

    /// Quotas with a finite absolute lifespan, for exercising the backstop cap.
    fn quotas_with_lifespan(max_lifespan_sec: u64) -> KvQuotas {
        KvQuotas {
            max_lifespan_sec,
            ..quotas()
        }
    }

    #[test]
    fn effective_expires_at_caps_and_forces_finite_lifetime() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

        // No lifespan cap: a TTL-less write stays eternal, a TTL write is honoured (here under the
        // 86_400 `max_ttl_sec`, so it is not clamped).
        assert_eq!(effective_expires_at(now, None, quotas()), None);
        assert_eq!(
            effective_expires_at(now, Some(3_600), quotas()),
            Some(now + TimeDuration::seconds(3_600))
        );

        // With a 7-day lifespan cap, a TTL-less write is forced to expire at the bound...
        let week = quotas_with_lifespan(7 * 24 * 3600);
        assert_eq!(
            effective_expires_at(now, None, week),
            Some(now + TimeDuration::days(7))
        );
        // ...a shorter requested TTL wins (capped first by `max_ttl_sec`, then by the lifespan)...
        assert_eq!(
            effective_expires_at(now, Some(3_600), week),
            Some(now + TimeDuration::seconds(3_600))
        );
        // ...and a longer requested TTL is clamped down to the lifespan ceiling. `max_ttl_sec`
        // (86_400) is itself below the 7-day lifespan, so the effective bound is one day here.
        assert_eq!(
            effective_expires_at(now, Some(30 * 24 * 3600), week),
            Some(now + TimeDuration::seconds(86_400))
        );
    }

    /// Generous quotas so individual tests can tighten only the dimension under test.
    fn quotas() -> KvQuotas {
        KvQuotas {
            max_key_bytes: 256,
            max_value_bytes: 4096,
            max_entries: 1000,
            max_total_bytes: 1_000_000,
            max_ttl_sec: 86_400,
            max_lifespan_sec: 0,
        }
    }

    /// Inserts a throw-away user + responder so KV rows satisfy their foreign keys, returning the
    /// responder id the store functions are scoped to.
    async fn seed_responder(pool: &PgPool) -> anyhow::Result<Uuid> {
        let user_id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO users (id, email, handle, created_at) VALUES ($1, $2, $3, now())"#,
            user_id,
            format!("{}@example.com", user_id.as_simple()),
            format!("h{}", user_id.as_simple())
        )
        .execute(pool)
        .await?;

        let responder_id = Uuid::new_v4();
        sqlx::query!(
            r#"
INSERT INTO user_data_webhooks_responders
    (user_id, id, name, location, method, enabled, settings, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now())
            "#,
            user_id,
            responder_id,
            format!("r{}", responder_id.as_simple()),
            "/webhook",
            &b""[..],
            true,
            &b""[..]
        )
        .execute(pool)
        .await?;

        Ok(responder_id)
    }

    #[sqlx::test]
    async fn get_set_delete_round_trip(pool: PgPool) -> anyhow::Result<()> {
        let responder_id = seed_responder(&pool).await?;

        assert_eq!(get_value(&pool, responder_id, "a").await?, None);

        set_value(&pool, responder_id, "a", b"hello", None, false, quotas())
            .await?
            .expect("write within quota");
        assert_eq!(
            get_value(&pool, responder_id, "a").await?,
            Some(b"hello".to_vec())
        );

        // Overwrite replaces the value (last-writer-wins without `if_absent`).
        set_value(&pool, responder_id, "a", b"world", None, false, quotas())
            .await?
            .expect("overwrite within quota");
        assert_eq!(
            get_value(&pool, responder_id, "a").await?,
            Some(b"world".to_vec())
        );

        assert_eq!(delete_value(&pool, responder_id, "a").await?, 1);
        assert_eq!(delete_value(&pool, responder_id, "a").await?, 0);
        assert_eq!(get_value(&pool, responder_id, "a").await?, None);

        Ok(())
    }

    #[sqlx::test]
    async fn get_entry_reports_expiry(pool: PgPool) -> anyhow::Result<()> {
        let responder_id = seed_responder(&pool).await?;

        // A TTL-less row reports no expiry.
        set_value(&pool, responder_id, "eternal", b"v", None, false, quotas())
            .await?
            .expect("write");
        let (value, expires_at) = get_entry(&pool, responder_id, "eternal")
            .await?
            .expect("row present");
        assert_eq!(value, b"v".to_vec());
        assert_eq!(expires_at, None);

        // A row with a future expiry reports it back (within a second of what we set).
        let deadline = OffsetDateTime::now_utc() + TimeDuration::days(7);
        set_value(
            &pool,
            responder_id,
            "ttl",
            b"v",
            Some(deadline),
            false,
            quotas(),
        )
        .await?
        .expect("write");
        let (_, reported) = get_entry(&pool, responder_id, "ttl")
            .await?
            .expect("row present");
        let reported = reported.expect("expiry present");
        assert!((reported - deadline).abs() < TimeDuration::seconds(1));

        Ok(())
    }

    #[sqlx::test]
    async fn scoping_isolates_responders(pool: PgPool) -> anyhow::Result<()> {
        let a = seed_responder(&pool).await?;
        let b = seed_responder(&pool).await?;

        set_value(&pool, a, "k", b"a-value", None, false, quotas())
            .await?
            .expect("write");

        // The same key under a different responder is independent.
        assert_eq!(get_value(&pool, b, "k").await?, None);
        set_value(&pool, b, "k", b"b-value", None, false, quotas())
            .await?
            .expect("write");

        assert_eq!(get_value(&pool, a, "k").await?, Some(b"a-value".to_vec()));
        assert_eq!(get_value(&pool, b, "k").await?, Some(b"b-value".to_vec()));

        Ok(())
    }

    #[sqlx::test]
    async fn if_absent_is_first_writer_wins(pool: PgPool) -> anyhow::Result<()> {
        let responder_id = seed_responder(&pool).await?;

        set_value(&pool, responder_id, "k", b"first", None, true, quotas())
            .await?
            .expect("first write");
        // A second `if_absent` write on a live row is a no-op (the original survives).
        set_value(&pool, responder_id, "k", b"second", None, true, quotas())
            .await?
            .expect("no-op write");
        assert_eq!(
            get_value(&pool, responder_id, "k").await?,
            Some(b"first".to_vec())
        );

        // An unconditional write still replaces it.
        set_value(&pool, responder_id, "k", b"third", None, false, quotas())
            .await?
            .expect("overwrite");
        assert_eq!(
            get_value(&pool, responder_id, "k").await?,
            Some(b"third".to_vec())
        );

        Ok(())
    }

    #[sqlx::test]
    async fn if_absent_reclaims_expired_rows(pool: PgPool) -> anyhow::Result<()> {
        let responder_id = seed_responder(&pool).await?;
        let past = OffsetDateTime::now_utc() - TimeDuration::seconds(60);

        set_value(
            &pool,
            responder_id,
            "k",
            b"stale",
            Some(past),
            true,
            quotas(),
        )
        .await?
        .expect("expired write");
        // The expired row is invisible, so `if_absent` is allowed to take the slot.
        assert_eq!(get_value(&pool, responder_id, "k").await?, None);

        set_value(&pool, responder_id, "k", b"fresh", None, true, quotas())
            .await?
            .expect("reclaim expired");
        assert_eq!(
            get_value(&pool, responder_id, "k").await?,
            Some(b"fresh".to_vec())
        );

        Ok(())
    }

    #[sqlx::test]
    async fn expired_rows_are_invisible(pool: PgPool) -> anyhow::Result<()> {
        let responder_id = seed_responder(&pool).await?;
        let past = OffsetDateTime::now_utc() - TimeDuration::seconds(1);
        let future = OffsetDateTime::now_utc() + TimeDuration::seconds(3600);

        set_value(
            &pool,
            responder_id,
            "live",
            b"v",
            Some(future),
            false,
            quotas(),
        )
        .await?
        .expect("live write");
        set_value(
            &pool,
            responder_id,
            "dead",
            b"v",
            Some(past),
            false,
            quotas(),
        )
        .await?
        .expect("expired write");

        assert_eq!(get_value(&pool, responder_id, "dead").await?, None);
        let entries = list_entries(&pool, responder_id, None, None, 100, false).await?;
        let keys: Vec<_> = entries.into_iter().map(|entry| entry.key).collect();
        assert_eq!(keys, vec!["live".to_string()]);

        Ok(())
    }

    #[sqlx::test]
    async fn set_rejects_oversized_key_and_value(pool: PgPool) -> anyhow::Result<()> {
        let responder_id = seed_responder(&pool).await?;
        let mut tight = quotas();
        tight.max_key_bytes = 4;
        tight.max_value_bytes = 4;

        let key_err = set_value(&pool, responder_id, "toolong", b"v", None, false, tight).await?;
        assert!(key_err.unwrap_err().contains("key exceeds"));

        let value_err = set_value(&pool, responder_id, "k", b"toolong", None, false, tight).await?;
        assert!(value_err.unwrap_err().contains("value exceeds"));

        // Nothing was persisted by the rejected writes.
        assert_eq!(
            list_entries(&pool, responder_id, None, None, 100, false)
                .await?
                .len(),
            0
        );

        Ok(())
    }

    #[sqlx::test]
    async fn set_enforces_entry_quota(pool: PgPool) -> anyhow::Result<()> {
        let responder_id = seed_responder(&pool).await?;
        let mut tight = quotas();
        tight.max_entries = 2;

        set_value(&pool, responder_id, "a", b"1", None, false, tight)
            .await?
            .expect("first entry");
        set_value(&pool, responder_id, "b", b"1", None, false, tight)
            .await?
            .expect("second entry");

        // A third distinct key trips the entry quota.
        let err = set_value(&pool, responder_id, "c", b"1", None, false, tight).await?;
        assert!(err.unwrap_err().contains("entry limit"));

        // Updating an existing key does not add an entry, so it still succeeds at the cap.
        set_value(&pool, responder_id, "a", b"2", None, false, tight)
            .await?
            .expect("update at cap");

        Ok(())
    }

    #[sqlx::test]
    async fn set_enforces_total_bytes_quota(pool: PgPool) -> anyhow::Result<()> {
        let responder_id = seed_responder(&pool).await?;
        let mut tight = quotas();
        tight.max_total_bytes = 8;

        set_value(&pool, responder_id, "a", b"aaaa", None, false, tight)
            .await?
            .expect("4 of 8 bytes");
        set_value(&pool, responder_id, "b", b"bbbb", None, false, tight)
            .await?
            .expect("8 of 8 bytes");

        // One more byte over the cap is rejected.
        let err = set_value(&pool, responder_id, "c", b"c", None, false, tight).await?;
        assert!(err.unwrap_err().contains("total size limit"));

        // Shrinking an existing key frees budget for the next write.
        set_value(&pool, responder_id, "a", b"a", None, false, tight)
            .await?
            .expect("shrink frees budget");
        set_value(&pool, responder_id, "c", b"cc", None, false, tight)
            .await?
            .expect("fits after shrink");

        Ok(())
    }

    #[sqlx::test]
    async fn list_entries_prefix_after_limit_and_values(pool: PgPool) -> anyhow::Result<()> {
        let responder_id = seed_responder(&pool).await?;
        for key in ["req/a", "req/b", "req/c", "other/x"] {
            set_value(
                &pool,
                responder_id,
                key,
                key.as_bytes(),
                None,
                false,
                quotas(),
            )
            .await?
            .expect("write");
        }

        // Prefix scoping, ascending key order.
        let entries =
            list_entries(&pool, responder_id, Some("req/".into()), None, 100, true).await?;
        let keys: Vec<_> = entries.iter().map(|entry| entry.key.clone()).collect();
        assert_eq!(keys, vec!["req/a", "req/b", "req/c"]);
        // Values are included and base64url-decoded back to the original bytes.
        assert_eq!(
            entries[0].value.as_deref().map(|v| {
                use base64ct::{Base64UrlUnpadded, Encoding};
                Base64UrlUnpadded::decode_vec(v).unwrap()
            }),
            Some(b"req/a".to_vec())
        );

        // `after` is an exclusive cursor.
        let after = list_entries(
            &pool,
            responder_id,
            Some("req/".into()),
            Some("req/a".into()),
            100,
            true,
        )
        .await?;
        let keys: Vec<_> = after.into_iter().map(|entry| entry.key).collect();
        assert_eq!(keys, vec!["req/b", "req/c"]);

        // `limit` caps the page.
        let limited = list_entries(&pool, responder_id, Some("req/".into()), None, 2, true).await?;
        assert_eq!(limited.len(), 2);

        // `values_included = false` omits the value payload.
        let no_values =
            list_entries(&pool, responder_id, Some("req/".into()), None, 100, false).await?;
        assert!(no_values.iter().all(|entry| entry.value.is_none()));

        Ok(())
    }

    #[test]
    fn set_opts_deserialize_camel_case() {
        let opts: SetOpts = serde_json::from_str(r#"{"ttlSec": 3600, "ifAbsent": true}"#).unwrap();
        assert_eq!(opts.ttl_sec, Some(3600));
        assert!(opts.if_absent);

        let empty: SetOpts = serde_json::from_str("{}").unwrap();
        assert_eq!(empty.ttl_sec, None);
        assert!(!empty.if_absent);
    }

    #[test]
    fn list_opts_deserialize_camel_case() {
        let opts: ListOpts = serde_json::from_str(
            r#"{"prefix": "req/", "after": "req/a", "limit": 50, "valuesIncluded": false}"#,
        )
        .unwrap();
        assert_eq!(opts.prefix.as_deref(), Some("req/"));
        assert_eq!(opts.after.as_deref(), Some("req/a"));
        assert_eq!(opts.limit, Some(50));
        assert_eq!(opts.values_included, Some(false));
    }

    #[test]
    fn wait_opts_deserialize_camel_case() {
        let opts: WaitOpts =
            serde_json::from_str(r#"{"prefix": "req/t/", "timeoutMs": 25000}"#).unwrap();
        assert_eq!(opts.prefix, "req/t/");
        assert_eq!(opts.timeout_ms, Some(25000));
        assert_eq!(opts.after, None);
    }
}
