#!/bin/bash
set -euo pipefail

cd $(git rev-parse --show-toplevel)

# .env is committed (PORT, DATABASE_URL); .env.secret is gitignored (API keys,
# tailnet hostnames) - same two-file split and `set -a` sourcing deploy_server.sh
# already uses, just applied to the local run instead of the deploy step.
set -a
[ -f .env ] && source .env
[ -f .env.secret ] && source .env.secret
set +a

(cd web && trunk build)
cargo run -p server
