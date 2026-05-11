// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Build script: compiles the WebUI (Next.js static export) so rust-embed can
//! bundle the assets.

use std::path::Path;
use std::process::Command;

fn main() {
    let webui_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../webui");

    // Re-run if webui sources change
    println!("cargo::rerun-if-changed=../webui/src");
    println!("cargo::rerun-if-changed=../webui/package.json");
    println!("cargo::rerun-if-changed=../webui/package-lock.json");
    println!("cargo::rerun-if-changed=../webui/next.config.ts");
    println!("cargo::rerun-if-changed=../webui/tsconfig.json");
    println!("cargo::rerun-if-changed=../webui/messages");

    // Install dependencies (deterministic from lockfile)
    let status = Command::new("npm")
        .arg("ci")
        .current_dir(&webui_dir)
        .status()
        .expect("failed to run `npm ci` — is npm installed?");
    assert!(status.success(), "`npm ci` failed with {status}");

    // Build the static export into webui/out/
    let status = Command::new("npm")
        .args(["run", "build"])
        .current_dir(&webui_dir)
        .status()
        .expect("failed to run `npm run build` — is npm installed?");
    assert!(status.success(), "`npm run build` failed with {status}");
}
