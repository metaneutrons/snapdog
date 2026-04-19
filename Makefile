.PHONY: setup check fmt clippy test dev build-webui build-all knxprod

# ── OpenKNXproducer ────────────────────────────────────────────
PRODUCER_VERSION := v4.3.5
PRODUCER_REPO    := https://github.com/OpenKNX/OpenKNXproducer
UNAME_S          := $(shell uname -s)
UNAME_M          := $(shell uname -m)
ifeq ($(UNAME_S),Darwin)
  ifeq ($(UNAME_M),arm64)
    DOTNET_RID := osx-arm64
  else
    DOTNET_RID := osx-x64
  endif
else
  DOTNET_RID := linux-x64
endif
PRODUCER_BIN := tools/OpenKNXproducer

$(PRODUCER_BIN):
	@echo "Building OpenKNXproducer $(PRODUCER_VERSION) for $(DOTNET_RID)..."
	@rm -rf /tmp/OpenKNXproducer-build
	git clone --depth 1 --branch $(PRODUCER_VERSION) $(PRODUCER_REPO) /tmp/OpenKNXproducer-build
	dotnet publish /tmp/OpenKNXproducer-build/OpenKNXproducer.csproj \
		-c Release -r $(DOTNET_RID) --self-contained true /p:PublishSingleFile=true -o tools/
	@rm -rf /tmp/OpenKNXproducer-build
	@echo "✅ OpenKNXproducer built: $(PRODUCER_BIN)"

## Generate SnapDog.knxprod from OpenKNXproducer XML
knxprod: $(PRODUCER_BIN)
	cd knx && ../$(PRODUCER_BIN) knxprod SnapDog.xml
	@echo "✅ knx/SnapDog.knxprod generated"

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
