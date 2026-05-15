#!/usr/bin/env bash
# dev-setup — configure a fresh clone of agentlint for local development.
set -eou pipefail

echo "→ configuring git hooks"
git config core.hooksPath hooks

echo "→ installing cargo tools"
cargo install cargo-nextest --locked 2>/dev/null || true
cargo install rustqual --locked 2>/dev/null || true

echo "✓ dev setup complete"
