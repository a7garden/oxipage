//! One operation slot per site, shared by build and deploy.
//!
//! `SiteOperationGuard` replaces the separate `BuildGuard`/`DeployGuard`:
//! only one build OR deploy may run per site at a time (different sites stay
//! concurrent). It also retains the terminal `OperationSnapshot` after the
//! operation finishes so a client can reconnect and see the final state.

use dashmap::DashMap;
use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SiteOperationKind {
    Build,
    Deploy,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationEvent {
    pub event: String,
    pub data: Value,
    pub terminal: bool,
}

impl OperationEvent {
    pub fn progress(event: impl Into<String>, data: Value) -> Self {
        Self {
            event: event.into(),
            data,
            terminal: false,
        }
    }
    pub fn terminal(event: impl Into<String>, data: Value) -> Self {
        Self {
            event: event.into(),
            data,
            terminal: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationSnapshot {
    pub kind: SiteOperationKind,
    pub run_id: String,
    pub active: bool,
    pub started_at: String,
    pub terminal: Option<Value>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct OperationConflict {
    pub kind: SiteOperationKind,
    pub run_id: String,
}

struct Slot {
    snapshot: OperationSnapshot,
    tx: tokio::sync::broadcast::Sender<OperationEvent>,
    /// CAS: the first caller (SSE subscriber or watchdog) to flip this
    /// false→true owns starting the operation. Avoids the event-loss race
    /// where the operation emits before any subscriber connects.
    started: AtomicBool,
}

/// Registry-level singleton tracking the single in-flight operation per site.
pub struct SiteOperationGuard {
    slots: DashMap<String, Slot>,
}

impl SiteOperationGuard {
    pub fn new() -> Self {
        Self {
            slots: DashMap::new(),
        }
    }

    /// Reserve an operation slot for `slug`. Returns the conflicting
    /// operation if one is already active for that site.
    pub fn try_start(
        &self,
        slug: &str,
        id: &str,
        kind: SiteOperationKind,
    ) -> Result<(), OperationConflict> {
        use dashmap::mapref::entry::Entry;
        match self.slots.entry(slug.into()) {
            Entry::Occupied(o) if o.get().snapshot.active => Err(OperationConflict {
                kind: o.get().snapshot.kind,
                run_id: o.get().snapshot.run_id.clone(),
            }),
            Entry::Occupied(mut o) => {
                let (tx, _) = tokio::sync::broadcast::channel(128);
                o.insert(Slot {
                    snapshot: OperationSnapshot {
                        kind,
                        run_id: id.into(),
                        active: true,
                        started_at: now(),
                        terminal: None,
                    },
                    tx,
                    started: AtomicBool::new(false),
                });
                Ok(())
            }
            Entry::Vacant(v) => {
                let (tx, _) = tokio::sync::broadcast::channel(128);
                v.insert(Slot {
                    snapshot: OperationSnapshot {
                        kind,
                        run_id: id.into(),
                        active: true,
                        started_at: now(),
                        terminal: None,
                    },
                    tx,
                    started: AtomicBool::new(false),
                });
                Ok(())
            }
        }
    }

    /// Current snapshot for `slug` — retained (with terminal state) after
    /// the operation finishes.
    pub fn current(&self, slug: &str) -> Option<OperationSnapshot> {
        self.slots.get(slug).map(|x| x.snapshot.clone())
    }

    /// New broadcast receiver for `slug`'s operation `id`.
    pub fn subscribe(
        &self,
        slug: &str,
        id: &str,
    ) -> Option<tokio::sync::broadcast::Receiver<OperationEvent>> {
        self.slots
            .get(slug)
            .filter(|x| x.snapshot.run_id == id)
            .map(|x| x.tx.subscribe())
    }

    /// Relay an event to subscribers; a terminal event also updates the
    /// retained snapshot's `terminal` payload.
    pub fn publish(&self, slug: &str, e: OperationEvent) -> Result<(), ()> {
        let mut x = self.slots.get_mut(slug).ok_or(())?;
        if e.terminal {
            x.snapshot.terminal = Some(e.data.clone());
        }
        let _ = x.tx.send(e);
        Ok(())
    }

    /// Mark the operation inactive (keeps the snapshot + terminal state).
    pub fn finish(&self, slug: &str) -> Result<(), ()> {
        self.slots.get_mut(slug).ok_or(())?.snapshot.active = false;
        Ok(())
    }

    /// CAS-claim the right to start the operation for `slug`. Returns `false`
    /// if another caller already owns starting it (or no slot exists).
    pub fn try_claim(&self, slug: &str) -> bool {
        use std::sync::atomic::Ordering;
        self.slots
            .get(slug)
            .map(|s| {
                s.started
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            })
            .unwrap_or(false)
    }
}

impl Default for SiteOperationGuard {
    fn default() -> Self {
        Self::new()
    }
}

fn now() -> String {
    // SQLite-style UTC timestamp; chrono isn't a console dependency.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            let secs = d.as_secs();
            // ISO-ish "YYYY-MM-DDTHH:MM:SSZ" from Unix epoch via days.
            let days = secs / 86400;
            let (y, mo, da) = civil_from_days(days as i64);
            let rem = secs % 86400;
            let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
            format!("{y:04}-{mo:02}-{da:02}T{h:02}:{mi:02}:{s:02}Z")
        })
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

/// Days → civil date (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
