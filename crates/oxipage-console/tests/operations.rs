//! Tests for the shared per-site operation guard.

use oxipage_console::operations::{OperationEvent, SiteOperationGuard, SiteOperationKind};

#[test]
fn conflicts_only_within_site() {
    let g = SiteOperationGuard::new();
    g.try_start("a", "b1", SiteOperationKind::Build).unwrap();
    let e = g
        .try_start("a", "d1", SiteOperationKind::Deploy)
        .unwrap_err();
    assert_eq!((e.kind, e.run_id), (SiteOperationKind::Build, "b1".into()));
    assert!(g.try_start("b", "d2", SiteOperationKind::Deploy).is_ok());
}

#[test]
fn terminal_state_survives_finish() {
    let g = SiteOperationGuard::new();
    g.try_start("a", "d1", SiteOperationKind::Deploy).unwrap();
    g.publish(
        "a",
        OperationEvent::terminal("deployed", serde_json::json!({ "url": "u" })),
    )
    .unwrap();
    g.finish("a").unwrap();
    let s = g.current("a").unwrap();
    assert!(!s.active);
    assert_eq!(s.terminal.unwrap()["url"], "u");
}

#[test]
fn claim_happens_once() {
    let g = SiteOperationGuard::new();
    g.try_start("a", "b1", SiteOperationKind::Build).unwrap();
    assert!(g.try_claim("a"));
    assert!(!g.try_claim("a"), "second claim must lose the CAS");
    assert!(!g.try_claim("missing"));
}

#[test]
fn subscribe_matches_run_id() {
    let g = SiteOperationGuard::new();
    g.try_start("a", "b1", SiteOperationKind::Build).unwrap();
    assert!(g.subscribe("a", "b1").is_some());
    assert!(g.subscribe("a", "other").is_none());
}
