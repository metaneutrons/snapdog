.PHONY: setup check fmt clippy test dev build-webui build-all knxprod

# ── KNX Product Database ───────────────────────────────────────

## Generate KNX ETS XML from group object definitions
knx-xml:
	cargo run -p xtask -- knx/SnapDog.xml

## Generate SnapDog.knxprod (cross-platform, no external tools)
knxprod: knx-xml
	cargo install --git https://github.com/metaneutrons/knx-rs knx-prod --locked 2>/dev/null || true
	knx-prod knx/SnapDog.xml -o knx/SnapDog.knxprod

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
