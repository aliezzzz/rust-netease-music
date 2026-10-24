#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all -- --check
G_DEBUG=fatal-criticals cargo test -p music-core
cargo check -p netease-cloud-music-desktop
npm run build
npm test
npm run test:e2e
