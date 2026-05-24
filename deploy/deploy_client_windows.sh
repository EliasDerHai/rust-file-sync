#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

PROJECT="client"
SERVICE_NAME="rust-file-sync_client"
SERVICE_DIR="C:/projects/code/backend/rust-file-sync_windows_service"
SERVICE_PATH="${SERVICE_DIR}/client.exe"
EXECUTABLE_NAME="client.exe"

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

# Ensure directories exist
mkdir -p "${SERVICE_DIR}/logs"

# Copy config template if no config exists
if [[ ! -f "${SERVICE_DIR}/config.yaml" ]]; then
    echo "No config found. Copying template to ${SERVICE_DIR}/config.yaml..."
    cp ../config.yaml.template "${SERVICE_DIR}/config.yaml"
    echo "IMPORTANT: Please edit ${SERVICE_DIR}/config.yaml with your settings before starting the service."
fi

# Register service with nssm on first setup
if ! nssm status "${SERVICE_NAME}" > /dev/null 2>&1; then
    echo "Registering service with nssm for the first time..."
    nssm install "${SERVICE_NAME}" "${SERVICE_PATH}"
    nssm set "${SERVICE_NAME}" AppDirectory "${SERVICE_DIR}"
    nssm set "${SERVICE_NAME}" AppStdout "${SERVICE_DIR}/logs/stdout.txt"
    nssm set "${SERVICE_NAME}" AppStderr "${SERVICE_DIR}/logs/stdout.txt"
fi

if yes_or_no "Do you want to bump the semantic version of the workspace's Cargo.toml"; then
  echo "Bumping version..."
  cargo run -p version-bump -- --toml ../Cargo.toml --semver patch
else
  echo "Skipping version bump."
fi

echo "Stopping local service via nssm..."
nssm stop "${SERVICE_NAME}" 2>/dev/null || echo "Service was not running."

status=$(nssm status "${SERVICE_NAME}" 2>/dev/null || echo "SERVICE_STOPPED")
if [[ "$status" != "SERVICE_STOPPED" ]]; then
  echo "nssm couldn't stop ${SERVICE_NAME} - still ${status}"
  exit 1
fi

cargo build -p ${PROJECT} --release
echo "Copying binary to ${SERVICE_PATH} ..."
cp ../target/release/${EXECUTABLE_NAME} "${SERVICE_PATH}"

echo "Starting local service via nssm..."
nssm start "${SERVICE_NAME}"

for i in {1..10}; do
  status=$(nssm status "${SERVICE_NAME}")
  if [[ "$status" == "SERVICE_RUNNING" ]]; then
    echo "Service ${SERVICE_NAME} is running."
    exit 0
  fi
  echo "nssm couldn't start ${SERVICE_NAME} - status is ${status} ($((10 - i)) attempts remaining)"
  sleep 2
done

echo "Error: giving up - you might wanna check ${SERVICE_DIR}/logs/stdout.txt"
exit 1
