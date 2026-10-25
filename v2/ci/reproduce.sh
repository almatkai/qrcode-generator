#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
cargo fmt --manifest-path v2/Cargo.toml --check
cargo test --locked --all-targets --manifest-path v2/Cargo.toml
cargo clippy --locked --all-targets --manifest-path v2/Cargo.toml -- -D warnings
cargo build --locked --release --manifest-path v2/Cargo.toml
python3 v2/qa/process-smoke.py v2/target/release/tezbar-qrcode-generator
npm ci --prefix v2/qa --ignore-scripts
node v2/qa/host-tests.cjs
# Immutable V1 reference extraction (do not copy dependencies or run commands in the original V1).
python3 v2/tools/oracle-setup.py
node v2/qa/source-oracle.cjs
node v2/qa/svg-check.cjs
cargo test --locked --manifest-path v2/Cargo.toml --test domain independently_rasterized_svg
