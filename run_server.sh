#!/bin/bash
set -euo pipefail

cd $(git rev-parse --show-toplevel)

( cd web && trunk build )
cargo run -p server
