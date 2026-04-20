.PHONY: setup check fmt clippy test dev build-webui build-all knxprod

# ── OpenKNXproducer ────────────────────────────────────────────
PRODUCER_VERSION := 4.3.5
PRODUCER_URL     := https://github.com/OpenKNX/OpenKNXproducer/releases/download/v$(PRODUCER_VERSION)/OpenKNXproducer-$(PRODUCER_VERSION).zip
UNAME_S          := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
  PRODUCER_PLATFORM := MacOS
else ifeq ($(UNAME_S),Linux)
  PRODUCER_PLATFORM := Linux
else
  PRODUCER_PLATFORM := Windows
endif
PRODUCER_BIN := tools/OpenKNXproducer

$(PRODUCER_BIN):
	@echo "Downloading OpenKNXproducer $(PRODUCER_VERSION) for $(PRODUCER_PLATFORM)..."
	@mkdir -p tools
	curl -sL "$(PRODUCER_URL)" -o /tmp/openknxproducer.zip
	unzip -jo /tmp/openknxproducer.zip "tools/$(PRODUCER_PLATFORM)/OpenKNXproducer*" -d tools/
	chmod +x tools/OpenKNXproducer*
	@rm -f /tmp/openknxproducer.zip
	@echo "✅ OpenKNXproducer $(PRODUCER_VERSION): $(PRODUCER_BIN)"

## Validate KNX product XML (runs on all platforms)
knxprod-check: $(PRODUCER_BIN)
	$(PRODUCER_BIN) create --NoXsd -d knx/SnapDog
	@echo "✅ KNX product XML validated"

## Generate SnapDog.knxprod (requires Windows + ETS DLLs)
knxprod: $(PRODUCER_BIN)
	$(PRODUCER_BIN) create --NoXsd knx/SnapDog
	@test -f knx/SnapDog.knxprod && echo "✅ knx/SnapDog.knxprod generated" || echo "⚠️  knxprod skipped (requires Windows + ETS)"

## First-time setup: configure git hooks
setup:
	git config core.hooksPath .githooks
	@echo "✅ Git hooks configured"

## Run all checks (same as CI)
check: fmt clippy test

fmt:
	cargo fmt -- --check

clippy:
	cargo clippy -- -D warnings

test:
	cargo test

## Build WebUI (Next.js static export)
build-webui:
	cd webui && npm ci && npm run build

## Build everything: WebUI then Rust binary
build-all: build-webui
	cargo build --release

## Start dev environment
dev:
	docker compose -f docker-compose.dev.yml up -d
	@echo "✅ Dev infrastructure running"
	@echo "   Run: cargo run -- --config snapdog.dev.toml"
