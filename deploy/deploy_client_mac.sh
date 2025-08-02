#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

PROJECT="client"
SERVICE_NAME="rust-file-sync_client"
SERVICE_PATH="/Users/eliashaider/Applications/rust-file-sync_client"
EXECUTABLE_NAME="client"

function yes_or_no {
    while true; do
        read -p "$* [y/n]: " yn
        case $yn in
            [Yy]*) return 0 ;;
            [Nn]*) echo "Aborted"; return 1 ;;
            *) echo "Please answer y or n." ;;
        esac
    done
}

if yes_or_no "Do you want to bump the semantic version of the workspace's Cargo.toml"; then
  echo "Bumping version..."
  cargo run -p version-bump -- --toml ../Cargo.toml --semver patch
else
  echo "Skipping version bump."
fi

echo "Stopping local service via launchctl..."
launchctl stop ${SERVICE_NAME}

cargo build -p ${PROJECT} --release
echo "Copying binary to ${SERVICE_PATH} ..."
cp ../target/release/client ${SERVICE_PATH}

echo "Starting local service via launchctl..."
launchctl start ${SERVICE_NAME}

# wait for service to start
sleep 1

echo "Checking service status..."
pid=$(launchctl list | grep "${SERVICE_NAME}" | awk '{print $1}' || echo "-")

if [[ "${pid}" != "-" && "${pid}" != "0" ]]; then
    echo "Service ${SERVICE_NAME} is running with PID ${pid}."
    if yes_or_no "Do you want to tail the logs?"; then
        tail -n 100 -f ${SERVICE_PATH}/logs/client.log
    fi
else
    echo "Error: Service ${SERVICE_NAME} failed to start."
    exit 1
fi

