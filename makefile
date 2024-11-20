# Makefile

# Build target
build:
	@echo "Building Ledger Suite Manager..."
	cargo build --release --target wasm32-unknown-unknown --package lsm
	candid-extractor target/wasm32-unknown-unknown/release/lsm.wasm > lsm.did


