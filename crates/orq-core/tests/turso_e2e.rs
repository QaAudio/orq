//! Live E2E against the Turso/libSQL remote backend.
//!
//! Credentials come from `tools/orq/.env` (gitignored) or the environment:
//! `ORQ_DB_URL` + `ORQ_DB_TOKEN`. When absent the test is skipped so CI and
//! offline machines stay green.
//!
//! Run explicitly with: `cargo test -p orq-core --test turso_e2e -- --nocapture`

use orq_core::store::Store;
use orq_core::types::{LeaseKind, StorageTier};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

fn load_env_file() {
    // tools/orq/.env relative to this crate (crates/orq-core).
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.env");
    let Ok(body) = std::fs::read_to_string(&path) else {
        return;
    };
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if std::env::var(k.trim()).is_err() {
                std::env::set_var(k.trim(), v.trim());
            }
        }
    }
}

fn creds() -> Option<(String, String)> {
    load_env_file();
    let url = std::env::var("ORQ_DB_URL").ok()?;
    let token = std::env::var("ORQ_DB_TOKEN").ok()?;
    if url.trim().is_empty() {
        return None;
    }
    Some((url.trim().to_string(), token.trim().to_string()))
}

#[test]
fn turso_remote_full_roundtrip() {
    let Some((url, token)) = creds() else {
        eprintln!("SKIP turso_remote_full_roundtrip: no ORQ_DB_URL/ORQ_DB_TOKEN");
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let store = Store::open_remote(dir.path(), &url, &token).expect("connect to Turso");
    assert!(store.is_remote());

    // Unique workspace so parallel/failed runs never collide.
    let ws = format!("e2e-{}", uuid::Uuid::new_v4().simple());

    // Ensure cleanup even on assert failure.
    struct Cleanup<'a>(&'a Store, String);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            if let Err(e) = self.0.drop_workspace(&self.1) {
                eprintln!("cleanup drop_workspace failed: {e}");
            }
        }
    }
    let _cleanup = Cleanup(&store, ws.clone());

    // Workspace + events.
    let w = store.ensure_workspace(&ws, None).unwrap();
    assert_eq!(w.name, ws);
    let events = store.list_events(&ws, None, 10, None).unwrap();
    assert!(
        events.iter().any(|e| e.kind == "workspace.created"),
        "expected workspace.created event, got {:?}",
        events.iter().map(|e| e.kind.clone()).collect::<Vec<_>>()
    );

    // POI table + set/get.
    store.create_poi_table(&ws, "cells", "mixed", vec![]).unwrap();
    let poi = store
        .set_poi(
            &ws,
            "cells",
            "alpha",
            json!({"v": 1}),
            HashMap::new(),
            Some("fresh"),
            StorageTier::Durable,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(poi.version, 1);
    assert_eq!(poi.state, "fresh");

    // CAS success then conflict.
    let poi2 = store
        .set_poi(
            &ws,
            "cells",
            "alpha",
            json!({"v": 2}),
            HashMap::new(),
            None,
            StorageTier::Durable,
            None,
            Some(1),
            None,
        )
        .unwrap();
    assert_eq!(poi2.version, 2);
    let conflict = store.set_poi(
        &ws,
        "cells",
        "alpha",
        json!({"v": 99}),
        HashMap::new(),
        None,
        StorageTier::Durable,
        None,
        Some(1), // stale version
        None,
    );
    assert!(conflict.is_err(), "stale CAS write must fail");

    // Write lease blocks a foreign writer, allows the holder.
    store
        .acquire_lease(&ws, "cells", "alpha", LeaseKind::Write, "agent-a", "editing", 60)
        .unwrap();
    let blocked = store.set_poi(
        &ws,
        "cells",
        "alpha",
        json!({"v": 3}),
        HashMap::new(),
        None,
        StorageTier::Durable,
        None,
        None,
        None, // no holder identity -> blocked
    );
    assert!(blocked.is_err(), "write lease must block anonymous writers");
    let allowed = store
        .set_poi(
            &ws,
            "cells",
            "alpha",
            json!({"v": 3}),
            HashMap::new(),
            None,
            StorageTier::Durable,
            None,
            None,
            Some("agent-a"),
        )
        .unwrap();
    assert_eq!(allowed.version, 3);
    store.release_lease(&ws, "cells", "alpha", "agent-a").unwrap();

    // Event log is ordered and includes the poi mutations.
    let events = store.list_events(&ws, None, 100, None).unwrap();
    let changed = events.iter().filter(|e| e.kind == "poi.changed").count();
    assert!(changed >= 3, "expected >=3 poi.changed events, got {changed}");
    let mut last = 0;
    for e in &events {
        assert!(e.id > last, "event ids must be strictly increasing");
        last = e.id;
    }

    // Dashboard snapshot builds against remote data too.
    let snap = orq_core::dash::build_snapshot(&store, &ws, None, 100).unwrap();
    assert_eq!(snap["workspace"], ws.as_str());
    assert!(snap["board"].as_array().map(|a| !a.is_empty()).unwrap_or(false));
    assert!(snap["events"].as_array().map(|a| !a.is_empty()).unwrap_or(false));

    println!("turso E2E ok: workspace={ws}, {} events", events.len());
}
