#!/bin/bash
set -euo pipefail

cd $(git rev-parse --show-toplevel)

cargo sqlx database create
cargo sqlx migrate run --source server/migrations
cargo build
