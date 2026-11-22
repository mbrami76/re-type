// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Michele Brami

// Embeds the application icon (a precompiled resource in assets/).
fn main() {
    println!("cargo:rerun-if-changed=assets/re-type.res");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        println!("cargo:rustc-link-arg-bin=re-type={dir}/assets/re-type.res");
    }
}
