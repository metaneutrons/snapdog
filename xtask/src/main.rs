// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Generate monolithic ETS XML for SnapDog KNX product database.
//!
//! Reads GO definitions from hardcoded constants (mirroring group_objects.rs)
//! and outputs a complete ETS-compatible XML that `knx-prod` can convert to .knxprod.

const AID: &str = "M-00FA_A-FF01-01-0000";
const MFR: &str = "M-00FA";
const MAX_ZONES: usize = 10;
const MAX_CLIENTS: usize = 10;
const MEMORY_SIZE: usize = 256; // Rounded up from actual usage (~173 bytes)

fn main() {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "knx/SnapDog.xml".into());
    let xml = generate_xml();
    std::fs::write(&output, xml).expect("failed to write XML");
    eprintln!(
        "✅ Generated {output} ({} zones × {} COs + {} clients × {} COs = {} COs)",
        MAX_ZONES,
        ZONE_GOS.len(),
        MAX_CLIENTS,
        CLIENT_GOS.len(),
        MAX_ZONES * ZONE_GOS.len() + MAX_CLIENTS * CLIENT_GOS.len()
    );
}

// ── GO definitions (mirrors group_objects.rs) ─────────────────

#[allow(dead_code)] // name_en used when adding TranslationUnit elements
struct Go {
    name_de: &'static str,
    name_en: &'static str,
    func: &'static str,
    dpt: &'static str,
    size: &'static str,
    read: bool,
    write: bool,
    transmit: bool,
    update: bool,
}

const fn recv(
    name_de: &'static str,
    name_en: &'static str,
    func: &'static str,
    dpt: &'static str,
    size: &'static str,
) -> Go {
    Go {
        name_de,
        name_en,
        func,
        dpt,
        size,
        read: false,
        write: true,
        transmit: false,
        update: false,
    }
}
const fn send(
    name_de: &'static str,
    name_en: &'static str,
    func: &'static str,
    dpt: &'static str,
    size: &'static str,
) -> Go {
    Go {
        name_de,
        name_en,
        func,
        dpt,
        size,
        read: true,
        write: false,
        transmit: true,
        update: false,
    }
}
const fn bidir(
    name_de: &'static str,
    name_en: &'static str,
    func: &'static str,
    dpt: &'static str,
    size: &'static str,
) -> Go {
    Go {
        name_de,
        name_en,
        func,
        dpt,
        size,
        read: true,
        write: true,
        transmit: true,
        update: true,
    }
}

const ZONE_GOS: &[Go] = &[
    recv("Play", "Play", "Play", "DPST-1-1", "1 Bit"),
    recv("Pause", "Pause", "Pause", "DPST-1-1", "1 Bit"),
    recv("Stop", "Stop", "Stop", "DPST-1-1", "1 Bit"),
    recv(
        "Nächster Titel",
        "Next Track",
        "Track Next",
        "DPST-1-1",
        "1 Bit",
    ),
    recv(
        "Vorheriger Titel",
        "Previous Track",
        "Track Previous",
        "DPST-1-1",
        "1 Bit",
    ),
    recv("Lautstärke", "Volume", "Volume", "DPST-5-1", "1 Byte"),
    send(
        "Lautstärke Status",
        "Volume Status",
        "Volume Status",
        "DPST-5-1",
        "1 Byte",
    ),
    recv(
        "Lautstärke Dimmen",
        "Volume Dim",
        "Volume Dim",
        "DPST-3-7",
        "4 Bit",
    ),
    recv("Stumm", "Mute", "Mute", "DPST-1-1", "1 Bit"),
    send(
        "Stumm Status",
        "Mute Status",
        "Mute Status",
        "DPST-1-1",
        "1 Bit",
    ),
    recv(
        "Stumm Umschalten",
        "Mute Toggle",
        "Mute Toggle",
        "DPST-1-1",
        "1 Bit",
    ),
    send(
        "Wiedergabe Status",
        "Playback Status",
        "Control Status",
        "DPST-1-1",
        "1 Bit",
    ),
    send(
        "Titel spielt",
        "Track Playing",
        "Track Playing",
        "DPST-1-1",
        "1 Bit",
    ),
    recv(
        "Zufallswiedergabe",
        "Shuffle",
        "Shuffle",
        "DPST-1-1",
        "1 Bit",
    ),
    send(
        "Zufall Status",
        "Shuffle Status",
        "Shuffle Status",
        "DPST-1-1",
        "1 Bit",
    ),
    recv(
        "Zufall Umschalten",
        "Shuffle Toggle",
        "Shuffle Toggle",
        "DPST-1-1",
        "1 Bit",
    ),
    recv("Wiederholung", "Repeat", "Repeat", "DPST-1-1", "1 Bit"),
    send(
        "Wiederholung Status",
        "Repeat Status",
        "Repeat Status",
        "DPST-1-1",
        "1 Bit",
    ),
    recv(
        "Wiederholung Umsch.",
        "Repeat Toggle",
        "Repeat Toggle",
        "DPST-1-1",
        "1 Bit",
    ),
    recv(
        "Titel Wiederholung",
        "Track Repeat",
        "Track Repeat",
        "DPST-1-1",
        "1 Bit",
    ),
    send(
        "Titel Wdh. Status",
        "Track Repeat Status",
        "Track Repeat Status",
        "DPST-1-1",
        "1 Bit",
    ),
    recv(
        "Titel Wdh. Umsch.",
        "Track Repeat Toggle",
        "Track Repeat Toggle",
        "DPST-1-1",
        "1 Bit",
    ),
    recv("Playlist", "Playlist", "Playlist", "DPST-5-10", "1 Byte"),
    send(
        "Playlist Status",
        "Playlist Status",
        "Playlist Status",
        "DPST-5-10",
        "1 Byte",
    ),
    recv(
        "Nächste Playlist",
        "Next Playlist",
        "Playlist Next",
        "DPST-1-1",
        "1 Bit",
    ),
    recv(
        "Vorherige Playlist",
        "Previous Playlist",
        "Playlist Previous",
        "DPST-1-1",
        "1 Bit",
    ),
    send(
        "Titel",
        "Track Title",
        "Track Title",
        "DPST-16-1",
        "14 Bytes",
    ),
    send(
        "Interpret",
        "Track Artist",
        "Track Artist",
        "DPST-16-1",
        "14 Bytes",
    ),
    send(
        "Album",
        "Track Album",
        "Track Album",
        "DPST-16-1",
        "14 Bytes",
    ),
    send(
        "Fortschritt",
        "Track Progress",
        "Track Progress",
        "DPST-5-1",
        "1 Byte",
    ),
    // Presence
    recv("Präsenz", "Presence", "Presence", "DPST-1-18", "1 Bit"),
    bidir(
        "Präsenz Aktiviert",
        "Presence Enable",
        "Presence Enable",
        "DPST-1-1",
        "1 Bit",
    ),
    bidir(
        "Präsenz Timeout",
        "Presence Timeout",
        "Presence Timeout",
        "DPST-7-5",
        "2 Bytes",
    ),
    send(
        "Präsenz Timer",
        "Presence Timer",
        "Presence Timer Active",
        "DPST-1-1",
        "1 Bit",
    ),
    recv(
        "Präsenz Quelle",
        "Presence Source",
        "Presence Source Override",
        "DPST-5-10",
        "1 Byte",
    ),
];

const CLIENT_GOS: &[Go] = &[
    recv("Lautstärke", "Volume", "Volume", "DPST-5-1", "1 Byte"),
    send(
        "Lautstärke Status",
        "Volume Status",
        "Volume Status",
        "DPST-5-1",
        "1 Byte",
    ),
    recv(
        "Lautstärke Dimmen",
        "Volume Dim",
        "Volume Dim",
        "DPST-3-7",
        "4 Bit",
    ),
    recv("Stumm", "Mute", "Mute", "DPST-1-1", "1 Bit"),
    send(
        "Stumm Status",
        "Mute Status",
        "Mute Status",
        "DPST-1-1",
        "1 Bit",
    ),
    recv(
        "Stumm Umschalten",
        "Mute Toggle",
        "Mute Toggle",
        "DPST-1-1",
        "1 Bit",
    ),
    recv("Latenz", "Latency", "Latency", "DPST-5-10", "1 Byte"),
    send(
        "Latenz Status",
        "Latency Status",
        "Latency Status",
        "DPST-5-10",
        "1 Byte",
    ),
    bidir(
        "Zonenzuordnung",
        "Zone Assignment",
        "Zone",
        "DPST-5-10",
        "1 Byte",
    ),
    send(
        "Zone Status",
        "Zone Status",
        "Zone Status",
        "DPST-5-10",
        "1 Byte",
    ),
    send("Verbunden", "Connected", "Connected", "DPST-1-1", "1 Bit"),
];

// ── Zone CO grouping for Dynamic section ──────────────────────

struct CoGroup {
    title_de: &'static str,
    title_en: &'static str,
    indices: &'static [usize],
}

const ZONE_GROUPS: &[CoGroup] = &[
    CoGroup {
        title_de: "Wiedergabe",
        title_en: "Playback",
        indices: &[0, 1, 2, 3, 4, 11, 12],
    },
    CoGroup {
        title_de: "Lautstärke",
        title_en: "Volume",
        indices: &[5, 6, 7, 8, 9, 10],
    },
    CoGroup {
        title_de: "Zufallswiedergabe / Wiederholung",
        title_en: "Shuffle / Repeat",
        indices: &[13, 14, 15, 16, 17, 18, 19, 20, 21],
    },
    CoGroup {
        title_de: "Playlist",
        title_en: "Playlist",
        indices: &[22, 23, 24, 25],
    },
    CoGroup {
        title_de: "Titelinformationen",
        title_en: "Track Info",
        indices: &[26, 27, 28, 29],
    },
    CoGroup {
        title_de: "Präsenz",
        title_en: "Presence",
        indices: &[30, 31, 32, 33, 34],
    },
];

const CLIENT_GROUPS: &[CoGroup] = &[
    CoGroup {
        title_de: "Lautstärke",
        title_en: "Volume",
        indices: &[0, 1, 2, 3, 4, 5],
    },
    CoGroup {
        title_de: "Latenz und Zone",
        title_en: "Latency and Zone",
        indices: &[6, 7, 8, 9],
    },
    CoGroup {
        title_de: "Status",
        title_en: "Status",
        indices: &[10],
    },
];

// ── XML generation ────────────────────────────────────────────

fn generate_xml() -> String {
    let mut x = String::with_capacity(128 * 1024);
    w(&mut x, r#"<?xml version="1.0" encoding="utf-8"?>"#);
    w(
        &mut x,
        &format!(
            r#"<KNX xmlns="http://knx.org/xml/project/20" CreatedBy="SnapDog xtask" ToolVersion="1.0">"#
        ),
    );
    w(&mut x, "  <ManufacturerData>");
    w(&mut x, &format!(r#"    <Manufacturer RefId="{MFR}">"#));

    write_catalog(&mut x);
    write_application_program(&mut x);
    write_hardware(&mut x);

    w(&mut x, "    </Manufacturer>");
    w(&mut x, "  </ManufacturerData>");
    w(&mut x, "</KNX>");
    x
}

fn write_catalog(x: &mut String) {
    w(x, "      <Catalog>");
    w(
        x,
        &format!(
            r#"        <CatalogSection Id="{MFR}_CS-SnapDog" Name="SnapDog" Number="SnapDog" DefaultLanguage="de-DE">"#
        ),
    );
    w(
        x,
        &format!(
            r#"          <CatalogItem Id="{MFR}_H-0xFF01-1_HP-FF01-01-0000_CI-0xFF01-1" Name="SnapDog" Number="1" ProductRefId="{MFR}_H-0xFF01-1_P-0xFF01" Hardware2ProgramRefId="{MFR}_H-0xFF01-1_HP-FF01-01-0000" DefaultLanguage="de-DE" />"#
        ),
    );
    w(x, "        </CatalogSection>");
    w(x, "      </Catalog>");
}

fn write_application_program(x: &mut String) {
    w(x, "      <ApplicationPrograms>");
    w(
        x,
        &format!(
            r#"        <ApplicationProgram Id="{AID}" ProgramType="ApplicationProgram" MaskVersion="MV-07B0" Name="SnapDog" LoadProcedureStyle="MergedProcedure" PeiType="0" DefaultLanguage="de-DE" DynamicTableManagement="false" Linkable="true" MinEtsVersion="5.0" IPConfig="Custom" ApplicationNumber="65281" ApplicationVersion="1" ReplacesVersions="0">"#
        ),
    );
    w(x, "          <Static>");

    write_code_segment(x);
    write_parameter_types(x);
    write_parameters(x);
    write_com_objects(x);
    write_tables(x);
    write_load_procedures(x);
    write_options(x);

    w(x, "          </Static>");
    write_dynamic(x);
    w(x, "        </ApplicationProgram>");
    w(x, "      </ApplicationPrograms>");
}

fn write_code_segment(x: &mut String) {
    w(x, "            <Code>");
    w(
        x,
        &format!(
            r#"              <RelativeSegment Id="{AID}_RS-04-00000" Name="Parameters" Offset="0" Size="{MEMORY_SIZE}" LoadStateMachine="4" />"#
        ),
    );
    w(x, "            </Code>");
}

fn write_parameter_types(x: &mut String) {
    w(x, "            <ParameterTypes>");
    // Bool
    pt_enum(x, "YesNo", 8, &[("Nein", 0), ("Ja", 1)]);
    // Text types
    pt_text(x, "Name", 160);
    pt_text(x, "Text20", 160);
    pt_text(x, "Text40", 320);
    pt_text(x, "Text60", 480);
    pt_text(x, "Text80", 640);
    pt_text(x, "MAC", 136); // 17 chars
    // Numeric
    pt_num(x, "Percent", 8, "unsignedInt", 0, 100);
    pt_num(x, "UInt8", 8, "unsignedInt", 0, 255);
    pt_num(x, "UInt16", 16, "unsignedInt", 0, 65535);
    // Enums
    pt_enum(
        x,
        "LogLevel",
        8,
        &[
            ("Error", 0),
            ("Warn", 1),
            ("Info", 2),
            ("Debug", 3),
            ("Trace", 4),
        ],
    );
    pt_enum(
        x,
        "SampleRate",
        8,
        &[("44100 Hz", 0), ("48000 Hz", 1), ("96000 Hz", 2)],
    );
    pt_enum(
        x,
        "BitDepth",
        8,
        &[("16 Bit", 0), ("24 Bit", 1), ("32 Bit", 2)],
    );
    pt_enum(
        x,
        "ZoneSelect",
        8,
        &[
            ("Zone 1", 1),
            ("Zone 2", 2),
            ("Zone 3", 3),
            ("Zone 4", 4),
            ("Zone 5", 5),
            ("Zone 6", 6),
            ("Zone 7", 7),
            ("Zone 8", 8),
            ("Zone 9", 9),
            ("Zone 10", 10),
        ],
    );
    w(x, "            </ParameterTypes>");
}

fn pt_enum(x: &mut String, name: &str, bits: u16, values: &[(&str, u16)]) {
    w(
        x,
        &format!(r#"              <ParameterType Id="{AID}_PT-{name}" Name="{name}">"#),
    );
    w(
        x,
        &format!(r#"                <TypeRestriction Base="Value" SizeInBit="{bits}">"#),
    );
    for (i, (text, val)) in values.iter().enumerate() {
        w(
            x,
            &format!(
                r#"                  <Enumeration Text="{text}" Value="{val}" Id="{AID}_PT-{name}_EN-{i}" />"#
            ),
        );
    }
    w(x, "                </TypeRestriction>");
    w(x, "              </ParameterType>");
}

fn pt_text(x: &mut String, name: &str, bits: u16) {
    w(
        x,
        &format!(r#"              <ParameterType Id="{AID}_PT-{name}" Name="{name}">"#),
    );
    w(
        x,
        &format!(r#"                <TypeText SizeInBit="{bits}" />"#),
    );
    w(x, "              </ParameterType>");
}

fn pt_num(x: &mut String, name: &str, bits: u16, typ: &str, min: u32, max: u32) {
    w(
        x,
        &format!(r#"              <ParameterType Id="{AID}_PT-{name}" Name="{name}">"#),
    );
    w(
        x,
        &format!(
            r#"                <TypeNumber SizeInBit="{bits}" Type="{typ}" minInclusive="{min}" maxInclusive="{max}" />"#
        ),
    );
    w(x, "              </ParameterType>");
}

fn write_parameters(x: &mut String) {
    w(x, "            <Parameters>");
    let mut off = 0usize;

    // ── Zone parameters ───────────────────────────────────────
    for z in 1..=MAX_ZONES {
        let p = format!("Z{z:02}");
        // Active flag (offset 0-9)
        param_mem(
            x,
            &p,
            "001",
            "Active",
            "YesNo",
            "Zone aktiv",
            "1",
            &mut off,
            8,
        );
    }
    for z in 1..=MAX_ZONES {
        let p = format!("Z{z:02}");
        param_mem(
            x,
            &p,
            "002",
            "DefVol",
            "Percent",
            "Standard-Lautstärke",
            "50",
            &mut off,
            8,
        );
    }
    for z in 1..=MAX_ZONES {
        let p = format!("Z{z:02}");
        param_mem(
            x,
            &p,
            "003",
            "MaxVol",
            "Percent",
            "Max. Lautstärke",
            "100",
            &mut off,
            8,
        );
    }
    for z in 1..=MAX_ZONES {
        let p = format!("Z{z:02}");
        param_mem(
            x,
            &p,
            "004",
            "AirPlay",
            "YesNo",
            "AirPlay aktiviert",
            "1",
            &mut off,
            8,
        );
    }
    for z in 1..=MAX_ZONES {
        let p = format!("Z{z:02}");
        param_mem(
            x,
            &p,
            "005",
            "Spotify",
            "YesNo",
            "Spotify aktiviert",
            "1",
            &mut off,
            8,
        );
    }
    for z in 1..=MAX_ZONES {
        let p = format!("Z{z:02}");
        param_mem(
            x,
            &p,
            "006",
            "PresEn",
            "YesNo",
            "Präsenz aktiviert",
            "0",
            &mut off,
            8,
        );
    }
    for z in 1..=MAX_ZONES {
        let p = format!("Z{z:02}");
        param_mem(
            x,
            &p,
            "007",
            "PresTO",
            "UInt16",
            "Präsenz Auto-Off (s)",
            "900",
            &mut off,
            16,
        );
    }
    for z in 1..=MAX_ZONES {
        let p = format!("Z{z:02}");
        param_mem(
            x,
            &p,
            "008",
            "SRate",
            "SampleRate",
            "Sample Rate",
            "1",
            &mut off,
            8,
        );
    }
    for z in 1..=MAX_ZONES {
        let p = format!("Z{z:02}");
        param_mem(
            x,
            &p,
            "009",
            "BitD",
            "BitDepth",
            "Bit Depth",
            "0",
            &mut off,
            8,
        );
    }

    // ── Client parameters ─────────────────────────────────────
    for c in 1..=MAX_CLIENTS {
        let p = format!("C{c:02}");
        param_mem(
            x,
            &p,
            "001",
            "Active",
            "YesNo",
            "Client aktiv",
            "1",
            &mut off,
            8,
        );
    }
    for c in 1..=MAX_CLIENTS {
        let p = format!("C{c:02}");
        param_mem(
            x,
            &p,
            "002",
            "DefZone",
            "ZoneSelect",
            "Standard-Zone",
            "1",
            &mut off,
            8,
        );
    }
    for c in 1..=MAX_CLIENTS {
        let p = format!("C{c:02}");
        param_mem(
            x,
            &p,
            "003",
            "DefVol",
            "Percent",
            "Standard-Lautstärke",
            "100",
            &mut off,
            8,
        );
    }
    for c in 1..=MAX_CLIENTS {
        let p = format!("C{c:02}");
        param_mem(
            x,
            &p,
            "004",
            "MaxVol",
            "Percent",
            "Max. Lautstärke",
            "100",
            &mut off,
            8,
        );
    }
    for c in 1..=MAX_CLIENTS {
        let p = format!("C{c:02}");
        param_mem(
            x,
            &p,
            "005",
            "DefLat",
            "UInt8",
            "Standard-Latenz (ms)",
            "0",
            &mut off,
            8,
        );
    }

    // ── Text parameters (not memory-backed, ETS-only) ─────────
    for z in 1..=MAX_ZONES {
        param_text(
            x,
            &format!("Z{z:02}"),
            "000",
            "Name",
            "Name",
            "Zonenname",
            &format!("Zone {z}"),
        );
    }
    for c in 1..=MAX_CLIENTS {
        param_text(
            x,
            &format!("C{c:02}"),
            "000",
            "Name",
            "Name",
            "Clientname",
            &format!("Client {c}"),
        );
    }
    for c in 1..=MAX_CLIENTS {
        param_text(
            x,
            &format!("C{c:02}"),
            "010",
            "MAC",
            "MAC",
            "MAC-Adresse",
            "",
        );
    }

    // ── Global parameters ─────────────────────────────────────
    param_mem(
        x,
        "G",
        "001",
        "HttpPort",
        "UInt16",
        "HTTP Port",
        "5555",
        &mut off,
        16,
    );
    param_mem(
        x,
        "G",
        "002",
        "LogLvl",
        "LogLevel",
        "Log Level",
        "2",
        &mut off,
        8,
    );
    param_text(x, "G", "010", "SubURL", "Text60", "Subsonic URL", "");
    param_text(x, "G", "011", "SubUser", "Text20", "Subsonic Benutzer", "");
    param_text(x, "G", "012", "SubPass", "Text20", "Subsonic Passwort", "");
    param_text(x, "G", "013", "MqttBrk", "Text40", "MQTT Broker", "");
    param_text(
        x,
        "G",
        "014",
        "MqttTop",
        "Text20",
        "MQTT Base Topic",
        "snapdog",
    );

    // ── Radio stations ────────────────────────────────────────
    for r in 1..=20usize {
        let p = format!("R{r:02}");
        param_text(x, &p, "000", "Name", "Text20", "Stationsname", "");
        param_text(x, &p, "001", "URL", "Text80", "Stream URL", "");
        param_mem(x, &p, "002", "Active", "YesNo", "Aktiv", "0", &mut off, 8);
    }

    w(x, "            </Parameters>");
    eprintln!("  Memory layout: {off} bytes used");
}

/// Emit a memory-backed parameter inside a Union.
fn param_mem(
    x: &mut String,
    prefix: &str,
    num: &str,
    name: &str,
    pt: &str,
    text: &str,
    default: &str,
    offset: &mut usize,
    bits: u16,
) {
    let bytes = (bits / 8) as usize;
    w(x, &format!(r#"              <Union SizeInBit="{bits}">"#));
    w(
        x,
        &format!(
            r#"                <Memory CodeSegment="{AID}_RS-04-00000" Offset="{}" BitOffset="0" />"#,
            *offset
        ),
    );
    w(
        x,
        &format!(
            r#"                <Parameter Id="{AID}_UP-{prefix}{num}" Name="{prefix}_{name}" Offset="0" BitOffset="0" ParameterType="{AID}_PT-{pt}" Text="{text}" Value="{default}" />"#
        ),
    );
    w(x, "              </Union>");
    *offset += bytes;
}

/// Emit a text parameter (not memory-backed).
fn param_text(
    x: &mut String,
    prefix: &str,
    num: &str,
    name: &str,
    pt: &str,
    text: &str,
    default: &str,
) {
    w(
        x,
        &format!(
            r#"              <Parameter Id="{AID}_P-{prefix}{num}" Name="{prefix}_{name}" ParameterType="{AID}_PT-{pt}" Text="{text}" Value="{default}" />"#
        ),
    );
}

fn write_com_objects(x: &mut String) {
    w(x, "            <ComObjectTable>");
    for z in 1..=MAX_ZONES {
        for (i, go) in ZONE_GOS.iter().enumerate() {
            let num = (z - 1) * ZONE_GOS.len() + i + 1;
            write_com_object(
                x,
                &format!("Z{z:02}{i:03}"),
                &format!("Zone {z} {}", go.func),
                go,
                num,
            );
        }
    }
    for c in 1..=MAX_CLIENTS {
        for (i, go) in CLIENT_GOS.iter().enumerate() {
            let num = MAX_ZONES * ZONE_GOS.len() + (c - 1) * CLIENT_GOS.len() + i + 1;
            write_com_object(
                x,
                &format!("C{c:02}{i:03}"),
                &format!("Client {c} {}", go.func),
                go,
                num,
            );
        }
    }
    w(x, "            </ComObjectTable>");
}

fn write_com_object(x: &mut String, id_suffix: &str, name: &str, go: &Go, number: usize) {
    let r = if go.read { "Enabled" } else { "Disabled" };
    let wr = if go.write { "Enabled" } else { "Disabled" };
    let t = if go.transmit { "Enabled" } else { "Disabled" };
    let u = if go.update { "Enabled" } else { "Disabled" };
    w(
        x,
        &format!(
            r#"              <ComObject Id="{AID}_O-{id_suffix}" Name="{name}" Number="{number}" Text="{}" FunctionText="{}" ObjectSize="{}" DatapointType="{}" Priority="Low" ReadFlag="{r}" WriteFlag="{wr}" CommunicationFlag="Enabled" TransmitFlag="{t}" UpdateFlag="{u}" />"#,
            go.name_de, go.func, go.size, go.dpt
        ),
    );
}

fn write_tables(x: &mut String) {
    w(x, r#"            <AddressTable MaxEntries="2047" />"#);
    w(x, r#"            <AssociationTable MaxEntries="2047" />"#);
}

fn write_load_procedures(x: &mut String) {
    w(x, "            <LoadProcedures>");
    w(x, r#"              <LoadProcedure MergeId="1">"#);
    w(
        x,
        &format!(
            r#"                <LdCtrlCompareProp InlineData="0000FF010100" ObjIdx="0" PropId="78">"#
        ),
    );
    w(
        x,
        &format!(r#"                  <OnError Cause="CompareMismatch" MessageRef="{AID}_M-1" />"#),
    );
    w(x, "                </LdCtrlCompareProp>");
    w(x, "              </LoadProcedure>");
    w(x, r#"              <LoadProcedure MergeId="2">"#);
    w(
        x,
        &format!(
            r#"                <LdCtrlRelSegment LsmIdx="4" Size="{MEMORY_SIZE}" Mode="1" Fill="0" AppliesTo="full" />"#
        ),
    );
    w(
        x,
        &format!(
            r#"                <LdCtrlRelSegment LsmIdx="4" Size="{MEMORY_SIZE}" Mode="0" Fill="0" AppliesTo="par" />"#
        ),
    );
    w(x, "              </LoadProcedure>");
    w(x, r#"              <LoadProcedure MergeId="4">"#);
    w(
        x,
        &format!(
            r#"                <LdCtrlWriteRelMem ObjIdx="4" Offset="0" Size="{MEMORY_SIZE}" Verify="true" AppliesTo="full,par" />"#
        ),
    );
    w(x, "              </LoadProcedure>");
    w(x, r#"              <LoadProcedure MergeId="7">"#);
    w(
        x,
        r#"                <LdCtrlLoadImageProp ObjIdx="4" PropId="27" />"#,
    );
    w(x, "              </LoadProcedure>");
    w(x, "            </LoadProcedures>");
    w(x, "            <Messages>");
    w(
        x,
        &format!(
            r#"              <Message Id="{AID}_M-1" Name="VersionMismatch" Text="Application and firmware version mismatch." />"#
        ),
    );
    w(x, "            </Messages>");
}

fn write_options(x: &mut String) {
    w(
        x,
        r#"            <Options TextParameterEncoding="iso-8859-15" SupportsExtendedMemoryServices="true" SupportsExtendedPropertyServices="true" />"#,
    );
}

fn write_dynamic(x: &mut String) {
    w(x, "          <Dynamic>");
    // Zones
    for z in 1..=MAX_ZONES {
        write_channel_block(
            x,
            "Zone",
            z,
            &format!("{AID}_UP-Z{z:02}001"),
            &format!("{AID}_P-Z{z:02}000"),
            ZONE_GOS,
            ZONE_GROUPS,
            &format!("Z{z:02}"),
            30,
        );
    }
    // Clients
    for c in 1..=MAX_CLIENTS {
        write_channel_block(
            x,
            "Client",
            c,
            &format!("{AID}_UP-C{c:02}001"),
            &format!("{AID}_P-C{c:02}000"),
            CLIENT_GOS,
            CLIENT_GROUPS,
            &format!("C{c:02}"),
            11,
        );
    }
    w(x, "          </Dynamic>");
}

fn write_channel_block(
    x: &mut String,
    prefix: &str,
    idx: usize,
    active_param_id: &str,
    name_param_id: &str,
    _gos: &[Go],
    groups: &[CoGroup],
    id_prefix: &str,
    _go_count: usize,
) {
    let active_ref = format!("{active_param_id}_R-{active_param_id}");
    let name_ref = format!("{name_param_id}_R-{name_param_id}");
    w(
        x,
        &format!(r#"            <choose ParamRefId="{active_ref}">"#),
    );
    w(x, r#"              <when test="1">"#);
    w(
        x,
        &format!(
            r#"                <ParameterBlock Id="{AID}_PB-{id_prefix}" Name="{prefix}{idx}" Text="{prefix} {idx}: {{{{0: ...}}}}" TextParameterRefId="{name_ref}" ShowInComObjectTree="true">"#
        ),
    );
    // Name parameter
    w(
        x,
        &format!(r#"                  <ParameterRefRef RefId="{name_ref}" />"#),
    );
    // CO groups
    for group in groups {
        w(
            x,
            &format!(
                r#"                  <ParameterSeparator Id="{AID}_PS-{id_prefix}-{}" Text="{}" UIHint="Headline" />"#,
                group.title_en.replace(' ', ""),
                group.title_de
            ),
        );
        for &i in group.indices {
            let co_id = format!("{AID}_O-{id_prefix}{i:03}");
            let num = if prefix == "Zone" {
                (idx - 1) * 35 + i + 1
            } else {
                MAX_ZONES * 35 + (idx - 1) * 11 + i + 1
            };
            w(
                x,
                &format!(r#"                  <ComObjectRefRef RefId="{co_id}_R-{num}" />"#),
            );
        }
    }
    w(x, "                </ParameterBlock>");
    w(x, "              </when>");
    w(x, "            </choose>");
}

fn write_hardware(x: &mut String) {
    w(x, "      <Hardware>");
    w(
        x,
        &format!(
            r#"        <Hardware Id="{MFR}_H-0xFF01-1" Name="SnapDog" SerialNumber="0xFF01" VersionNumber="1" BusCurrent="0" HasIndividualAddress="true" HasApplicationProgram="true">"#
        ),
    );
    w(x, "          <Products>");
    w(
        x,
        &format!(
            r#"            <Product Id="{MFR}_H-0xFF01-1_P-0xFF01" Text="SnapDog" OrderNumber="0xFF01" IsRailMounted="false" DefaultLanguage="de-DE">"#
        ),
    );
    w(x, r#"              <RegistrationInfo />"#);
    w(x, "            </Product>");
    w(x, "          </Products>");
    w(x, "          <Hardware2Programs>");
    w(
        x,
        &format!(
            r#"            <Hardware2Program Id="{MFR}_H-0xFF01-1_HP-FF01-01-0000" MediumTypes="MT-0">"#
        ),
    );
    w(
        x,
        &format!(r#"              <ApplicationProgramRef RefId="{AID}" />"#),
    );
    w(x, "            </Hardware2Program>");
    w(x, "          </Hardware2Programs>");
    w(x, "        </Hardware>");
    w(x, "      </Hardware>");
}

fn w(s: &mut String, line: &str) {
    s.push_str(line);
    s.push('\n');
}
