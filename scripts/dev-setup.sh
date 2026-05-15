#!/usr/bin/env bash
# dev-setup — configure a fresh clone of agentlint for local development.
set -euo pipefail

echo "→ configuring git hooks"
git config core.hooksPath hooks

echo "→ installing cargo tools"
cargo install cargo-nextest --locked \
    || echo "warning: cargo-nextest install failed — install manually"
cargo install rustqual --locked \
    || echo "warning: rustqual install failed — install manually"

echo "✓ dev setup complete"
