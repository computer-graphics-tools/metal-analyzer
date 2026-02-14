use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use tracing::info;

/// Lightweight runtime counters for go-to-definition request behavior.
///
/// These counters let us estimate whether incremental DB infrastructure would
/// likely pay off, based on cache-hit profile and request latency.
#[derive(Default)]
pub(super) struct GotoDefPerf {
    requests: AtomicU64,
    hits: AtomicU64,
    misses: AtomicU64,
    memory_hits: AtomicU64,
    disk_hits: AtomicU64,
    ast_dump_hits: AtomicU64,
    total_elapsed_ns: AtomicU64,
}

impl GotoDefPerf {
    pub(super) fn record(
        &self,
        elapsed: Duration,
        index_source: Option<&'static str>,
        has_result: bool,
    ) {
        let requests = self.requests.fetch_add(1, Ordering::Relaxed) + 1;
        if has_result {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }

        match index_source {
            Some("memory") => {
                self.memory_hits.fetch_add(1, Ordering::Relaxed);
            },
            Some("disk") => {
                self.disk_hits.fetch_add(1, Ordering::Relaxed);
            },
            Some("ast_dump") => {
                self.ast_dump_hits.fetch_add(1, Ordering::Relaxed);
            },
            _ => {},
        }

        let elapsed_ns = elapsed.as_nanos().min(u64::MAX as u128) as u64;
        self.total_elapsed_ns.fetch_add(elapsed_ns, Ordering::Relaxed);

        if requests % 200 == 0 {
            self.log_summary();
        }
    }

    pub(super) fn log_summary(&self) {
        let requests = self.requests.load(Ordering::Relaxed);
        if requests == 0 {
            info!("[perf][goto-def] no requests recorded yet");
            return;
        }

        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let memory_hits = self.memory_hits.load(Ordering::Relaxed);
        let disk_hits = self.disk_hits.load(Ordering::Relaxed);
        let ast_dump_hits = self.ast_dump_hits.load(Ordering::Relaxed);
        let total_elapsed_ns = self.total_elapsed_ns.load(Ordering::Relaxed);

        let avg_ms = total_elapsed_ns as f64 / requests as f64 / 1_000_000.0;
        let ast_dump_ratio = ast_dump_hits as f64 / requests as f64;

        let recommendation = if ast_dump_ratio < 0.03 && avg_ms < 25.0 {
            "Incremental DB likely not justified yet; current cache hit profile is strong."
        } else if ast_dump_ratio > 0.15 || avg_ms > 75.0 {
            "Incremental DB may be justified; cold-path cost is significant."
        } else {
            "Profile is mixed; collect more usage data before committing to incremental DB work."
        };

        info!(
            "[perf][goto-def] requests={requests}, hits={hits}, misses={misses}, \
             source(memory={memory_hits}, disk={disk_hits}, ast_dump={ast_dump_hits}), \
             avg_ms={avg_ms:.2}. {recommendation}"
        );
    }
}
