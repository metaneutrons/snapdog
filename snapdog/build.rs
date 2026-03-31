// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Build script: compiles vendored libshairplay from C sources.

use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let base = manifest_dir.join("../vendor/shairplay/src/lib");
    let include = manifest_dir.join("../vendor/shairplay/include");

    cc::Build::new()
        .include(&base)
        .include(base.join("alac"))
        .include(base.join("crypto"))
        .include(base.join("curve25519"))
        .include(base.join("ed25519"))
        .include(base.join("playfair"))
        .include(&include)
        .include(include.join("shairplay"))
        // Core RAOP
        .file(base.join("raop.c"))
        .file(base.join("raop_buffer.c"))
        .file(base.join("raop_rtp.c"))
        // HTTP/RTSP
        .file(base.join("http_parser.c"))
        .file(base.join("http_request.c"))
        .file(base.join("http_response.c"))
        .file(base.join("httpd.c"))
        // DNS-SD
        .file(base.join("dnssd.c"))
        // Crypto
        .file(base.join("rsakey.c"))
        .file(base.join("rsapem.c"))
        .file(base.join("digest.c"))
        .file(base.join("aes_ctr.c"))
        .file(base.join("crypto/aes.c"))
        .file(base.join("crypto/bigint.c"))
        .file(base.join("crypto/hmac.c"))
        .file(base.join("crypto/md5.c"))
        .file(base.join("crypto/rc4.c"))
        .file(base.join("crypto/sha1.c"))
        // ALAC decoder
        .file(base.join("alac/alac.c"))
        // Curve25519 + Ed25519
        .file(base.join("curve25519/curve25519-donna.c"))
        .file(base.join("ed25519/fe.c"))
        .file(base.join("ed25519/ge.c"))
        .file(base.join("ed25519/keypair.c"))
        .file(base.join("ed25519/sc.c"))
        .file(base.join("ed25519/seed.c"))
        .file(base.join("ed25519/sha512.c"))
        .file(base.join("ed25519/sign.c"))
        .file(base.join("ed25519/verify.c"))
        .file(base.join("ed25519/add_scalar.c"))
        .file(base.join("ed25519/key_exchange.c"))
        // Pairing + FairPlay
        .file(base.join("pairing.c"))
        .file(base.join("fairplay_dummy.c"))
        // Utilities
        .file(base.join("base64.c"))
        .file(base.join("logger.c"))
        .file(base.join("netutils.c"))
        .file(base.join("plist.c"))
        .file(base.join("sdp.c"))
        .file(base.join("utils.c"))
        // Playfair
        .file(base.join("playfair/hand_garble.c"))
        .file(base.join("playfair/modified_md5.c"))
        .file(base.join("playfair/omg_hax.c"))
        .file(base.join("playfair/playfair.c"))
        .file(base.join("playfair/sap_hash.c"))
        // Compiler settings
        .warnings(false)
        .compile("shairplay");

    // Link against dns_sd (Bonjour on macOS, Avahi on Linux)
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    #[cfg(not(target_os = "macos"))]
    println!("cargo:rustc-link-lib=dylib=dns_sd");
    println!("cargo:rerun-if-changed={}", base.display());
}
