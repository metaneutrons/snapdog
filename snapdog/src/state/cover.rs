// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Cover art cache — in-memory store for zone cover images.
//! Served via GET /api/v1/zones/{id}/cover with correct MIME type.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

/// Thread-safe cover cache handle.
pub type SharedCoverCache = Arc<RwLock<CoverCache>>;

/// Create a new empty shared cover cache.
pub fn new_cache() -> SharedCoverCache {
    Arc::new(RwLock::new(CoverCache::default()))
}

/// In-memory store mapping zone indices to their current cover art.
#[derive(Default)]
pub struct CoverCache {
    entries: HashMap<usize, CoverEntry>,
}

/// A single cached cover image with its MIME type and content hash.
pub struct CoverEntry {
    /// Raw image bytes.
    pub bytes: Vec<u8>,
    /// MIME type (e.g. `image/jpeg`).
    pub mime: String,
    /// CRC32 hex hash for ETag / cache validation.
    pub hash: String,
}

impl CoverEntry {
    fn new(bytes: Vec<u8>, mime: String) -> Self {
        let hash = format!("{:08x}", crc32fast::hash(&bytes));
        Self { bytes, mime, hash }
    }
}

impl CoverCache {
    /// Store cover art for a zone.
    pub fn set(&mut self, zone_index: usize, bytes: Vec<u8>, mime: String) {
        self.entries
            .insert(zone_index, CoverEntry::new(bytes, mime));
    }

    /// Store cover art with auto-detected MIME from magic bytes.
    pub fn set_auto_mime(&mut self, zone_index: usize, bytes: Vec<u8>) {
        let mime = detect_mime(&bytes).to_string();
        self.entries
            .insert(zone_index, CoverEntry::new(bytes, mime));
    }

    /// Get cover art for a zone.
    pub fn get(&self, zone_index: usize) -> Option<&CoverEntry> {
        self.entries.get(&zone_index)
    }

    /// Clear cover art for a zone.
    pub fn clear(&mut self, zone_index: usize) {
        self.entries.remove(&zone_index);
    }
}

/// Simple percent-decoding for data: URI payloads.
fn percent_decode_bytes(input: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().and_then(|c| (c as char).to_digit(16));
            let lo = chars.next().and_then(|c| (c as char).to_digit(16));
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
            }
        } else {
            out.push(b);
        }
    }
    out
}
/// Detect image MIME type from magic bytes. Falls back to `application/octet-stream`.
pub fn detect_mime(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg",
        [0x89, 0x50, 0x4E, 0x47, ..] => "image/png",
        [
            0x52,
            0x49,
            0x46,
            0x46,
            _,
            _,
            _,
            _,
            0x57,
            0x45,
            0x42,
            0x50,
            ..,
        ] => "image/webp",
        _ => "application/octet-stream",
    }
}

/// Fetch cover art from a URL, returning (bytes, mime).
pub async fn fetch_cover(url: &str) -> Option<(Vec<u8>, String)> {
    // Handle data: URIs (e.g. data:image/svg+xml;charset=US-ASCII,%3Csvg...)
    if let Some(rest) = url.strip_prefix("data:") {
        let (header, data) = rest.split_once(',')?;
        let mime = header
            .split(';')
            .next()
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = percent_decode_bytes(data);
        return Some((bytes, mime));
    }

    let client = reqwest::Client::builder()
        .user_agent(crate::USER_AGENT)
        .build()
        .ok()?;
    let resp = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let mime = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let bytes = resp.bytes().await.ok()?.to_vec();
    Some((bytes, mime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_jpeg() {
        assert_eq!(detect_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), "image/jpeg");
    }

    #[test]
    fn detects_png() {
        assert_eq!(detect_mime(&[0x89, 0x50, 0x4E, 0x47]), "image/png");
    }

    #[test]
    fn detects_webp() {
        let webp = [0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50];
        assert_eq!(detect_mime(&webp), "image/webp");
    }

    #[test]
    fn unknown_fallback() {
        assert_eq!(detect_mime(&[0x00, 0x01]), "application/octet-stream");
    }

    #[test]
    fn cache_set_get_clear() {
        let mut cache = CoverCache::default();
        cache.set(1, vec![0xFF, 0xD8], "image/jpeg".into());
        assert!(cache.get(1).is_some());
        assert_eq!(cache.get(1).unwrap().mime, "image/jpeg");
        cache.clear(1);
        assert!(cache.get(1).is_none());
    }
}
