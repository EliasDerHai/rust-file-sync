#!/bin/bash
set -euo pipefail

cd $(git rev-parse --show-toplevel) 

touch ./server/comptime.db
cargo sqlx migrate run

cargo build
