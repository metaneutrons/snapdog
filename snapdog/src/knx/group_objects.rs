// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Group object definitions — single source of truth for all 410 KNX communication objects.
//!
//! Used by:
//! - Device mode runtime (BAU `GroupObjectStore` construction)
//! - `cargo xtask generate-knxprod-xml` (OpenKNXproducer XML generation)

use knx_core::dpt::{DPT_SCALING, DPT_STRING_8859_1, DPT_SWITCH, DPT_VALUE_1_UCOUNT, Dpt};

/// Maximum number of zones supported.
pub const MAX_ZONES: usize = 10;

/// Maximum number of clients supported.
pub const MAX_CLIENTS: usize = 10;

/// Number of group objects per zone.
pub const ZONE_GO_COUNT: usize = ZONE_GOS.len();

/// Number of group objects per client.
pub const CLIENT_GO_COUNT: usize = CLIENT_GOS.len();

/// Total number of group objects.
pub const TOTAL_GO_COUNT: usize = MAX_ZONES * ZONE_GO_COUNT + MAX_CLIENTS * CLIENT_GO_COUNT;

/// KNX communication object flags (matching ETS flag bits).
pub struct GoFlags {
    /// Communication enabled (K-flag).
    pub communicate: bool,
    /// Read enabled (L-flag) — bus can read this object.
    pub read: bool,
    /// Write enabled (S-flag) — bus can write this object.
    pub write: bool,
    /// Transmit on change (Ü-flag) — send to bus when value changes.
    pub transmit: bool,
    /// Update on response (A-flag) — update value from GroupValueResponse.
    pub update: bool,
}

/// Shorthand flag sets.
const RECV: GoFlags = GoFlags {
    communicate: true,
    read: false,
    write: true,
    transmit: false,
    update: false,
};
const SEND: GoFlags = GoFlags {
    communicate: true,
    read: true,
    write: false,
    transmit: true,
    update: false,
};
const BIDIR: GoFlags = GoFlags {
    communicate: true,
    read: true,
    write: true,
    transmit: true,
    update: true,
};

/// DPT 3.007 — Controlled dimming.
const DPT_CONTROL_DIMMING: Dpt = Dpt::new(3, 7);

/// Definition of a single group object.
pub struct GoDefinition {
    /// Human-readable name (used in ETS and logs).
    pub name: &'static str,
    /// KNX datapoint type.
    pub dpt: Dpt,
    /// Communication flags.
    pub flags: GoFlags,
}

impl GoFlags {
    /// Encode as a 16-bit group object descriptor (upper bits).
    pub const fn to_descriptor_bits(&self, size_code: u8) -> u16 {
        let mut bits: u16 = 0;
        if self.communicate {
            bits |= 1 << 10;
        }
        if self.read {
            bits |= 1 << 11;
        }
        if self.write {
            bits |= 1 << 12;
        }
        if self.transmit {
            bits |= 1 << 14;
        }
        if self.update {
            bits |= 1 << 15;
        }
        bits | (size_code as u16)
    }
}

// ── Zone group objects (30 per zone) ──────────────────────────

pub const ZONE_GOS: &[GoDefinition] = &[
    // Transport commands (receive only)
    GoDefinition {
        name: "Play",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Pause",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Stop",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Track Next",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Track Previous",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    // Volume
    GoDefinition {
        name: "Volume",
        dpt: DPT_SCALING,
        flags: RECV,
    },
    GoDefinition {
        name: "Volume Status",
        dpt: DPT_SCALING,
        flags: SEND,
    },
    GoDefinition {
        name: "Volume Dim",
        dpt: DPT_CONTROL_DIMMING,
        flags: RECV,
    },
    // Mute
    GoDefinition {
        name: "Mute",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Mute Status",
        dpt: DPT_SWITCH,
        flags: SEND,
    },
    GoDefinition {
        name: "Mute Toggle",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    // Playback status
    GoDefinition {
        name: "Control Status",
        dpt: DPT_SWITCH,
        flags: SEND,
    },
    GoDefinition {
        name: "Track Playing Status",
        dpt: DPT_SWITCH,
        flags: SEND,
    },
    // Shuffle
    GoDefinition {
        name: "Shuffle",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Shuffle Status",
        dpt: DPT_SWITCH,
        flags: SEND,
    },
    GoDefinition {
        name: "Shuffle Toggle",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    // Repeat (playlist)
    GoDefinition {
        name: "Repeat",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Repeat Status",
        dpt: DPT_SWITCH,
        flags: SEND,
    },
    GoDefinition {
        name: "Repeat Toggle",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    // Repeat (single track)
    GoDefinition {
        name: "Track Repeat",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Track Repeat Status",
        dpt: DPT_SWITCH,
        flags: SEND,
    },
    GoDefinition {
        name: "Track Repeat Toggle",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    // Playlist
    GoDefinition {
        name: "Playlist",
        dpt: DPT_VALUE_1_UCOUNT,
        flags: RECV,
    },
    GoDefinition {
        name: "Playlist Status",
        dpt: DPT_VALUE_1_UCOUNT,
        flags: SEND,
    },
    GoDefinition {
        name: "Playlist Next",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Playlist Previous",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    // Track metadata (send only)
    GoDefinition {
        name: "Track Title",
        dpt: DPT_STRING_8859_1,
        flags: SEND,
    },
    GoDefinition {
        name: "Track Artist",
        dpt: DPT_STRING_8859_1,
        flags: SEND,
    },
    GoDefinition {
        name: "Track Album",
        dpt: DPT_STRING_8859_1,
        flags: SEND,
    },
    GoDefinition {
        name: "Track Progress",
        dpt: DPT_SCALING,
        flags: SEND,
    },
];

// ── Client group objects (11 per client) ──────────────────────

pub const CLIENT_GOS: &[GoDefinition] = &[
    GoDefinition {
        name: "Volume",
        dpt: DPT_SCALING,
        flags: RECV,
    },
    GoDefinition {
        name: "Volume Status",
        dpt: DPT_SCALING,
        flags: SEND,
    },
    GoDefinition {
        name: "Volume Dim",
        dpt: DPT_CONTROL_DIMMING,
        flags: RECV,
    },
    GoDefinition {
        name: "Mute",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Mute Status",
        dpt: DPT_SWITCH,
        flags: SEND,
    },
    GoDefinition {
        name: "Mute Toggle",
        dpt: DPT_SWITCH,
        flags: RECV,
    },
    GoDefinition {
        name: "Latency",
        dpt: DPT_VALUE_1_UCOUNT,
        flags: RECV,
    },
    GoDefinition {
        name: "Latency Status",
        dpt: DPT_VALUE_1_UCOUNT,
        flags: SEND,
    },
    GoDefinition {
        name: "Zone",
        dpt: DPT_VALUE_1_UCOUNT,
        flags: BIDIR,
    },
    GoDefinition {
        name: "Zone Status",
        dpt: DPT_VALUE_1_UCOUNT,
        flags: SEND,
    },
    GoDefinition {
        name: "Connected Status",
        dpt: DPT_SWITCH,
        flags: SEND,
    },
];

/// Compute the 1-based ASAP for a zone group object.
///
/// Zone `zone_index` (1-based), GO `go_index` (0-based within `ZONE_GOS`).
pub const fn zone_asap(zone_index: usize, go_index: usize) -> u16 {
    ((zone_index - 1) * ZONE_GO_COUNT + go_index + 1) as u16
}

/// Compute the 1-based ASAP for a client group object.
///
/// Client `client_index` (1-based), GO `go_index` (0-based within `CLIENT_GOS`).
pub const fn client_asap(client_index: usize, go_index: usize) -> u16 {
    (MAX_ZONES * ZONE_GO_COUNT + (client_index - 1) * CLIENT_GO_COUNT + go_index + 1) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_go_count() {
        assert_eq!(ZONE_GOS.len(), 30);
    }

    #[test]
    fn client_go_count() {
        assert_eq!(CLIENT_GOS.len(), 11);
    }

    #[test]
    fn total_go_count() {
        assert_eq!(TOTAL_GO_COUNT, 410);
    }

    #[test]
    fn zone_asap_layout() {
        // Zone 1, first GO → ASAP 1
        assert_eq!(zone_asap(1, 0), 1);
        // Zone 1, last GO → ASAP 30
        assert_eq!(zone_asap(1, 29), 30);
        // Zone 2, first GO → ASAP 31
        assert_eq!(zone_asap(2, 0), 31);
        // Zone 10, last GO → ASAP 300
        assert_eq!(zone_asap(10, 29), 300);
    }

    #[test]
    fn client_asap_layout() {
        // Client 1, first GO → ASAP 301
        assert_eq!(client_asap(1, 0), 301);
        // Client 1, last GO → ASAP 311
        assert_eq!(client_asap(1, 10), 311);
        // Client 10, last GO → ASAP 410
        assert_eq!(client_asap(10, 10), 410);
    }

    #[test]
    fn no_asap_overlap() {
        let last_zone = zone_asap(MAX_ZONES, ZONE_GO_COUNT - 1);
        let first_client = client_asap(1, 0);
        assert_eq!(last_zone + 1, first_client);
    }

    #[test]
    fn recv_flags() {
        assert!(RECV.write);
        assert!(RECV.communicate);
        assert!(!RECV.read);
        assert!(!RECV.transmit);
    }

    #[test]
    fn send_flags() {
        assert!(SEND.read);
        assert!(SEND.transmit);
        assert!(SEND.communicate);
        assert!(!SEND.write);
    }

    #[test]
    fn descriptor_bits_recv() {
        // RECV: communicate(10) + write(12) = 0x1400 | size
        let bits = RECV.to_descriptor_bits(1);
        assert_eq!(bits, (1 << 10) | (1 << 12) | 1);
    }

    #[test]
    fn descriptor_bits_send() {
        // SEND: communicate(10) + read(11) + transmit(14) = 0x4C00 | size
        let bits = SEND.to_descriptor_bits(1);
        assert_eq!(bits, (1 << 10) | (1 << 11) | (1 << 14) | 1);
    }
}
