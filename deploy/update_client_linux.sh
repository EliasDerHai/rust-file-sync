#!/bin/bash
# Updates the rust-file-sync client binary from the latest GitHub release.
# Run deploy_client_linux.sh first for initial setup (systemd unit, directories, config).
#
# To release a new version:
#   cargo run -p version-bump -- --toml Cargo.toml --semver patch
#   git add Cargo.toml && git commit -m "bump version"
#   VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
#   git tag "v${VERSION}" && git push && git push --tags
set -euo pipefail

SERVICE_NAME="rust-file-sync-client"
SERVICE_PATH="${HOME}/.local/bin"
EXECUTABLE_NAME="client"
REPO="EliasDerHai/rust-file-sync"
ASSET="client-linux-x86_64"

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
systemctl --user stop "${SERVICE_NAME}" 2>/dev/null || true

echo "Installing binary to ${SERVICE_PATH}/${EXECUTABLE_NAME}..."
mv "${TMP}" "${SERVICE_PATH}/${EXECUTABLE_NAME}"

echo "Starting service..."
systemctl --user start "${SERVICE_NAME}"

sleep 1

echo "Checking service status..."
if systemctl --user is-active --quiet "${SERVICE_NAME}"; then
    echo "Service ${SERVICE_NAME} updated to ${LATEST} and running."
else
    echo "Error: Service ${SERVICE_NAME} failed to start."
    systemctl --user status "${SERVICE_NAME}" --no-pager || true
    exit 1
fi
