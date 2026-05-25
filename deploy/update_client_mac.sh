#!/bin/bash
# Updates the rust-file-sync client binary from the latest GitHub release.
# Run deploy_client_mac.sh first for initial setup (launchd plist, directories, config).
#
# To release a new version:
#   cargo run -p version-bump -- --toml Cargo.toml --semver patch
#   git add Cargo.toml && git commit -m "bump version"
#   VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
#   git tag "v${VERSION}" && git push && git push --tags
set -euo pipefail

SERVICE_NAME="rust-file-sync_client"
SERVICE_DIR="/Users/eliashaider/Applications/rust-file-sync_client"
EXECUTABLE_NAME="client"
REPO="EliasDerHai/rust-file-sync"
ASSET="client-macos-arm64"
PLIST_PATH="${HOME}/Library/LaunchAgents/${SERVICE_NAME}.plist"

if [[ ! -f "${PLIST_PATH}" ]]; then
    echo "Error: Launchd plist not found at ${PLIST_PATH}."
    echo "Run deploy_client_mac.sh first to set up the service."
    exit 1
fi

echo "Fetching latest release..."
LATEST=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | jq -r '.tag_name')

if [[ -z "${LATEST}" || "${LATEST}" == "null" ]]; then
    echo "Error: Could not determine latest release version."
    exit 1
fi

echo "Downloading ${EXECUTABLE_NAME} ${LATEST}..."
TMP=$(mktemp)
curl -fSL "https://github.com/${REPO}/releases/download/${LATEST}/${ASSET}" -o "${TMP}"
chmod +x "${TMP}"

echo "Stopping service..."
launchctl stop "${SERVICE_NAME}" 2>/dev/null || true

echo "Installing binary to ${SERVICE_DIR}/${EXECUTABLE_NAME}..."
mv "${TMP}" "${SERVICE_DIR}/${EXECUTABLE_NAME}"

echo "Starting service..."
launchctl start "${SERVICE_NAME}"

sleep 1

echo "Checking service status..."
pid=$(launchctl list | grep "${SERVICE_NAME}" | awk '{print $1}' || echo "-")

if [[ "${pid}" != "-" && "${pid}" != "0" ]]; then
    echo "Service ${SERVICE_NAME} updated to ${LATEST} and running with PID ${pid}."
else
    echo "Error: Service ${SERVICE_NAME} failed to start after update."
    exit 1
fi
