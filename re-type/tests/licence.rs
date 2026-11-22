// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

//! Every source file carries the copyright and licence line (REQ-LIC-001).

use std::path::Path;

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_rust_file_has_the_licence_line() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    walk(&root.join("src"), &mut files);
    walk(&root.join("tests"), &mut files);
    files.push(root.join("build.rs"));
    assert!(files.len() > 10);
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        let head: String = text.lines().take(3).collect::<Vec<_>>().join("\n");
        assert!(head.contains("SPDX-License-Identifier: GPL-3.0-or-later"), "{} has no licence line", f.display());
        assert!(head.contains("Copyright (C) 2026 Michele Brami"), "{} has no copyright line", f.display());
    }
}

#[test]
fn manifest_declares_the_licence() {
    let manifest = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    assert!(manifest.contains("license = \"GPL-3.0-or-later\""));
}
