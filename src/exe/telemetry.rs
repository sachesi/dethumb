use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FallbackReason {
    UnsupportedPlatform,
    NoIconAvailable,
    UnsupportedIconFormat,
    InvalidPeFormat,
    PermissionDenied,
    Io,
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExeTelemetrySnapshot {
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub extraction_attempts: u64,
    pub extraction_successes: u64,
    pub fallback_reasons: BTreeMap<FallbackReason, u64>,
}

fn telemetry_store() -> &'static Mutex<ExeTelemetrySnapshot> {
    static STORE: OnceLock<Mutex<ExeTelemetrySnapshot>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(ExeTelemetrySnapshot::default()))
}

fn with_store_mut<F>(f: F)
where
    F: FnOnce(&mut ExeTelemetrySnapshot),
{
    if let Ok(mut guard) = telemetry_store().lock() {
        f(&mut guard);
    }
}

pub fn record_cache_hit() {
    with_store_mut(|snapshot| snapshot.cache_hits += 1);
}

pub fn record_cache_miss() {
    with_store_mut(|snapshot| snapshot.cache_misses += 1);
}

pub fn record_extraction_attempt() {
    with_store_mut(|snapshot| snapshot.extraction_attempts += 1);
}

pub fn record_extraction_success() {
    with_store_mut(|snapshot| snapshot.extraction_successes += 1);
}

pub fn record_fallback_reason(reason: FallbackReason) {
    with_store_mut(|snapshot| {
        *snapshot.fallback_reasons.entry(reason).or_insert(0) += 1;
    });
}

#[must_use]
pub fn snapshot() -> ExeTelemetrySnapshot {
    telemetry_store()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

#[cfg(test)]
pub fn reset() {
    with_store_mut(|snapshot| *snapshot = ExeTelemetrySnapshot::default());
}
