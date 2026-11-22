// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami
//! Control identifiers of the settings window are unique: two controls with the same id would
//! make every lookup by id find the wrong one.

#[test]
fn settings_control_ids_are_unique_and_labels_do_not_collide() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/platform/windows/settings.rs")).unwrap();
    let mut ids: Vec<(String, i32)> = Vec::new();
    for line in src.lines() {
        let Some(rest) = line.strip_prefix("const ID_") else { continue };
        let Some((name, value)) = rest.split_once(": i32 = ") else { continue };
        if let Ok(v) = value.trim_end_matches(';').trim().parse::<i32>() {
            ids.push((name.to_string(), v));
        }
    }
    assert!(ids.len() > 40);
    for (i, (n, v)) in ids.iter().enumerate() {
        for (m, w) in &ids[i + 1..] {
            assert_ne!(v, w, "{n} and {m} share the id {v}");
            // labels use id + 1000 (see LBL)
            assert_ne!(v + 1000, *w, "label of {n} collides with {m}");
            assert_ne!(*v, w + 1000, "label of {m} collides with {n}");
        }
    }
}
