# Implementation Plan: knx-device Integration

**Branch:** `feat/knx-device` (from `refactor/knx-rs-migration`)
**Feature flag:** `knx-device` (disabled by default)

## Context

SnapDog currently operates as a KNX client (connects outward to a gateway).
This plan adds an optional "device mode" where SnapDog becomes a programmable
KNX/IP device with 410 group objects, discoverable and configurable by ETS.

### Prerequisites (done)

- [x] knx-rs migration (knx-core + knx-ip replace knxkit)
- [x] DPT corrections (5.010 for indices, 3.007 for dimming)

### Architecture

```
[knx]
enabled = true
mode = "client"                     # or "device"
url = "udp://192.168.1.50:3671"     # client mode only
individual_address = "1.1.100"      # device mode only
```

```
            ┌─────────────┐     ┌──────────────┐
            │ Client Mode │     │ Device Mode  │
            │ Multiplexer │     │ DeviceServer │
            │ + GroupOps  │     │ + BAU + GOs  │
            └──────┬──────┘     └──────┬───────┘
                   │                   │
                   └───────┬───────────┘
                           │
                ┌──────────▼──────────┐
                │   KnxTransport      │
                │  write_bool(ga,v)   │
                │  write_percent(ga,v)│
                │  on_group_write()   │
                └──────────┬──────────┘
                           │
                ┌──────────▼──────────┐
                │  Publisher/Listener  │
                │  (shared logic)      │
                └─────────────────────┘
```

### Group Object Inventory

- 10 zones × 30 GOs = 300
- 10 clients × 11 GOs = 110
- **Total: 410 group objects**

### SSOT Principle

GO definitions live once in Rust (`group_objects.rs`). Everything else is derived:

```
group_objects.rs  ──→  BAU GroupObjectStore (runtime)
       │
       └──→  cargo xtask generate-knxprod-xml
                    │
                    ▼
              knx/SnapDog.xml  (committed, reviewable)
                    │
                    ▼
              OpenKNXproducer knxprod
                    │
                    ▼
              knx/SnapDog.knxprod  (CI artifact, not committed)
```

---

## Task 1: Config schema

Add `mode` and `individual_address` to `KnxConfig`.

```rust
pub struct KnxConfig {
    pub enabled: bool,
    pub mode: String,                        // "client" (default) or "device"
    pub url: Option<String>,                 // client mode
    pub individual_address: Option<String>,  // device mode
    pub persist_ets_config: Option<bool>,    // device mode, default true
}
```

- Validate: device mode requires `individual_address`, client mode requires `url`
- Update `snapdog.example.toml`
- Tests: config parsing with both modes, validation errors for missing fields

## Task 2: Feature flag `knx-device`

- Add `knx-device` to workspace deps (git, optional)
- Add feature `knx-device = ["dep:knx-device"]` to snapdog/Cargo.toml
- Gate device-mode code behind `#[cfg(feature = "knx-device")]`
- Tests: builds with and without the feature

## Task 3: `KnxTransport` trait

Extract the transport abstraction from the current monolithic `knx/mod.rs`.

```rust
// snapdog/src/knx/transport.rs
#[allow(async_fn_in_trait)]
pub(crate) trait KnxTransport: Send {
    async fn write_bool(&self, ga: &str, value: bool);
    async fn write_percent(&self, ga: &str, value: u8);
    async fn write_u8(&self, ga: &str, value: u8);
    async fn write_string(&self, ga: &str, value: &str);
    async fn recv_group_write(&mut self) -> Option<(GroupAddress, Vec<u8>)>;
}
```

- Implement `ClientTransport` wrapping current `MultiplexHandle` + `GroupOps`
- Refactor publisher to use `&dyn KnxTransport`
- Refactor listener to use `recv_group_write()` loop
- Tests: all 20 existing KNX tests still pass (behavior unchanged)

## Task 4: Group object model (SSOT)

Create `snapdog/src/knx/group_objects.rs` — the single source of truth for
all GO definitions. Used by both the runtime BAU and the XML generator.

```rust
pub struct GoDefinition {
    pub name: &'static str,
    pub dpt: Dpt,
    pub flags: GoFlags,  // Read, Write, Communicate, Transmit, Update
}

pub const ZONE_GOS: &[GoDefinition] = &[
    GoDefinition { name: "Play",           dpt: DPT_SWITCH,          flags: WRITE },
    GoDefinition { name: "Pause",          dpt: DPT_SWITCH,          flags: WRITE },
    // ... 30 total
    GoDefinition { name: "Track Title",    dpt: DPT_STRING_8859_1,   flags: READ | TRANSMIT },
];

pub const CLIENT_GOS: &[GoDefinition] = &[
    GoDefinition { name: "Volume",         dpt: DPT_SCALING,         flags: WRITE },
    // ... 11 total
];
```

- Function `build_bau(config) -> Bau` creates BAU with 410 GOs, address table,
  association table from TOML config
- Tests: correct GO count, DPT assignment, table structure

## Task 5: `DeviceTransport`

`#[cfg(feature = "knx-device")]` gated implementation of `KnxTransport`.

- Wraps `DeviceServer` + `Bau`
- `write_*`: encode DPT → `go.set_value_if_changed()` → `bau.poll()` →
  `server.send_frame()` for outgoing frames
- `recv_group_write()`: `server.recv()` → `bau.process_frame()` →
  poll `group_objects.next_updated()` → return (GA, data)
- Spawns BAU poll loop as background task
- Tests: unit test with mock frames

## Task 6: Wire up device mode in `knx::start()`

```rust
match config.knx.mode.as_str() {
    "client" => {
        let transport = ClientTransport::connect(&config).await?;
        spawn_bridge(transport, ...);
    }
    #[cfg(feature = "knx-device")]
    "device" => {
        let transport = DeviceTransport::start(&config).await?;
        spawn_bridge(transport, ...);
    }
    _ => bail!("Unknown KNX mode"),
}
```

- Both paths feed into shared publisher/listener via `KnxTransport`
- Tests: integration test with device mode config starts without error

## Task 7: `cargo xtask generate-knxprod-xml`

New workspace member `xtask/` that generates `knx/SnapDog.xml` from the
GO definitions in `group_objects.rs`.

```
xtask/
├── Cargo.toml
└── src/
    └── main.rs    # subcommand: generate-knxprod-xml
```

- Reads `ZONE_GOS` and `CLIENT_GOS` constants
- Generates OpenKNX XML schema with:
  - Manufacturer 0xFA (OpenKNX Community)
  - Hardware + firmware description
  - 410 ComObjects with correct Name, Number, DPT, Flags
  - 10 zone channels + 10 client channels
  - Optional parameter: "Number of active zones" (1–10)
- Output: `knx/SnapDog.xml` (committed, reviewable, diffable)
- Tests: generated XML validates against OpenKNXproducer XSD
- Run: `cargo xtask generate-knxprod-xml`

## Task 8: OpenKNXproducer build + knxprod generation

### Makefile targets

```makefile
PRODUCER_VERSION := v4.3.5
PRODUCER_BIN     := tools/OpenKNXproducer

$(PRODUCER_BIN):
    git clone --depth 1 --branch $(PRODUCER_VERSION) \
        https://github.com/OpenKNX/OpenKNXproducer /tmp/OpenKNXproducer
    dotnet publish /tmp/OpenKNXproducer/OpenKNXproducer.csproj \
        -c Release -r $$(uname -m | sed 's/arm64/osx-arm64/;s/x86_64/linux-x64/') \
        --self-contained true /p:PublishSingleFile=true -o $(dir $@)
    rm -rf /tmp/OpenKNXproducer

knxprod: $(PRODUCER_BIN) knx/SnapDog.xml
    $(PRODUCER_BIN) knxprod knx/SnapDog.xml
```

### .gitignore additions

```
tools/
knx/*.knxprod
```

### CI workflow

```yaml
- uses: actions/cache@v4
  id: producer-cache
  with:
    path: tools/OpenKNXproducer
    key: openknxproducer-v4.3.5-linux-x64

- uses: actions/setup-dotnet@v4
  if: steps.producer-cache.outputs.cache-hit != 'true'
  with:
    dotnet-version: '9.0'

- name: Build OpenKNXproducer
  if: steps.producer-cache.outputs.cache-hit != 'true'
  run: make tools/OpenKNXproducer

- name: Generate knxprod
  run: make knxprod

- uses: actions/upload-artifact@v4
  with:
    name: snapdog-knxprod
    path: knx/SnapDog.knxprod
```

- Tests: `make knxprod` produces `knx/SnapDog.knxprod`
- Demo: `.knxprod` importable in ETS (manual verification)

## Task 9: ETS override logic

When device mode is active and ETS writes address/association tables:

1. BAU receives MemoryWrite frames from ETS tunnel connection
2. `bau.handle_memory_write()` stores data in memory area
3. `bau.load_tables_from_memory()` applies ETS tables, overriding TOML-derived tables
4. Memory area persisted to `knx-memory.bin` for survival across restarts
5. On startup: if `knx-memory.bin` exists and `persist_ets_config = true`,
   load it instead of building tables from TOML

- Tests: simulate ETS MemoryWrite → verify table changes → verify persistence
- Config: `persist_ets_config = true` (default in device mode)

## Task 10: Documentation and cleanup

- Update README KNX section with device mode docs
- Update `snapdog.example.toml` with device mode comments
- Add ADR for client vs device mode decision
- Remove `PLAN-KNX-RS.md` (completed)
- Tests: `cargo doc -p snapdog` builds without warnings

---

## Execution Order

```
Task 1 (config) ──→ Task 2 (feature flag) ──→ Task 3 (transport trait)
                                                       │
                                              ┌────────┴────────┐
                                              ▼                 ▼
                                    Task 4 (GO model)    Task 7 (xtask XML gen)
                                              │                 │
                                              ▼                 ▼
                                    Task 5 (DeviceTransport)  Task 8 (OpenKNXproducer)
                                              │
                                              ▼
                                    Task 6 (wire up)
                                              │
                                              ▼
                                    Task 9 (ETS override)
                                              │
                                              ▼
                                    Task 10 (docs)
```

Tasks 4+7 and 5+8 can be parallelized.

## Risk Assessment

- **Low**: Config changes, feature flag, transport trait — mechanical refactoring
- **Medium**: GO model + DeviceTransport — new code, but well-defined by knx-device API
- **Medium**: OpenKNXproducer XML schema — needs to match ETS expectations exactly
- **Low**: ETS override — BAU handles MemoryWrite natively, we just persist
- **External**: OpenKNXproducer .NET dependency — mitigated by caching + pinned version
