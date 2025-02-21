#!/bin/bash
set -euo pipefail

PROJECT="client"
SERVICE_NAME="rust-file-sync_client"
SERVICE_PATH="/Users/eliashaider/Documents/obsidian-vault/obsidian-vault"

echo "Stopping local service via nssm..."
launchctl stop ${SERVICE_NAME};

cargo build -p ${PROJECT} --release
echo "Copying binary to ${SERVICE_PATH} ..."
cp ../target/release/client ${SERVICE_PATH}

echo "Starting local service via nssm..."
launchctl start ${SERVICE_NAME};

