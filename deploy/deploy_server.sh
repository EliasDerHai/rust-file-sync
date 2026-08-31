#!/bin/bash
# Deploy the server to the Raspberry Pi (aarch64 Linux) via SSH.
#
# Pick how the binary is produced:
#   1) GitHub artifact  - download the latest published release (fastest; no
#                         local toolchain needed). Deploys a RELEASED version,
#                         not your working tree. Frontend is already embedded
#                         in the artifact by CI, so no trunk/wasm build here.
#   2) Build locally    - plain cargo build for the Pi target, using the
#                         cross-linker from .cargo/config.toml. Needs
#                         aarch64-linux-musl-gcc on PATH. Deploys your current 
#                         working tree.
#   3) Build with docker- cross (docker) build. Slowest but most hermetic.
#                         Deploys your current working tree.
#
# All paths target aarch64-unknown-linux-musl (statically linked), matching the
# release artifact so the three sources are interchangeable.
set -euo pipefail

cd "$(dirname "$0")"

# Single source of truth for PORT is the workspace .env (same file/pattern
# sqlx-macros already reads at compile time for DATABASE_URL). REMOTE_HOST
# lives in .env.secret instead (gitignored) - it's your tailnet hostname, not
# something to publish in this public repo.
set -a
[ -f ../.env ] && source ../.env
[ -f ../.env.secret ] && source ../.env.secret
set +a
PORT="${PORT:-3000}"
: "${REMOTE_HOST:?REMOTE_HOST not set - copy ../.env.secret.example to ../.env.secret and fill it in}"

REMOTE_USER="pi"
REMOTE_PATH="/home/pi/Downloads/Rust-File-Sync_Server"
SERVICE_NAME="rust-file-sync_server.service"
PROJECT="server"
TARGET="aarch64-unknown-linux-musl"
REPO="EliasDerHai/rust-file-sync"
RELEASE_ASSET="server-linux-aarch64"

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

# ---- pick build method -------------------------------------------------------
echo "How do you want to produce the server binary?"
echo "  1) GitHub artifact (latest release) - fastest, no local build"
echo "  2) Build locally"
echo "  3) Build with docker (cross)"
read -p "Select [1/2/3]: " METHOD

case "$METHOD" in
1)
    BINARY_PATH="../target/deploy-download/server"
    if ! command -v gh >/dev/null 2>&1; then
        echo "Error: 'gh' CLI is required for the GitHub-artifact path. Install it or pick 2/3."
        exit 1
    fi
    echo "Downloading latest '${RELEASE_ASSET}' from ${REPO}..."
    mkdir -p "$(dirname "$BINARY_PATH")"
    gh release download \
        --repo "$REPO" \
        --pattern "$RELEASE_ASSET" \
        --output "$BINARY_PATH" \
        --clobber
    chmod +x "$BINARY_PATH"
    TARGET_VERSION=$(gh release view --repo "$REPO" --json tagName -q .tagName 2>/dev/null | sed 's/^v//')
    ;;

2)
    # Plain cargo build, honoring the cross-linker in .cargo/config.toml
    # (aarch64-linux-musl-gcc). Works on a host that has that cross-gcc
    # installed. On macOS use option 1 or 3 instead.
    BINARY_PATH="../target/${TARGET}/release/server"
    if yes_or_no "Bump the semantic version of the workspace's Cargo.toml"; then
        echo "Bumping version..."
        cargo run -p version-bump -- --toml ../Cargo.toml --semver patch
    else
        echo "Skipping version bump."
    fi
    echo "Building web assets..."
    (cd ../web && trunk build --release)
    echo "Building ${PROJECT} for ${TARGET}..."
    cargo build -p "${PROJECT}" --release --target="${TARGET}"
    TARGET_VERSION=$(grep '^version' ../Cargo.toml | head -1 | cut -d'"' -f2)
    ;;

3)
    BINARY_PATH="../target/${TARGET}/release/server"
    if yes_or_no "Bump the semantic version of the workspace's Cargo.toml"; then
        echo "Bumping version..."
        cargo run -p version-bump -- --toml ../Cargo.toml --semver patch
    else
        echo "Skipping version bump."
    fi
    echo "Building web assets..."
    (cd ../web && trunk build --release)
    echo "Building ${PROJECT} for ${TARGET} (cross)..."
    if [[ "${MSYSTEM-}" == "MINGW64" ]]; then
        winpty cross build -p "${PROJECT}" --release --target="${TARGET}"
    else
        cross build -p "${PROJECT}" --release --target="${TARGET}"
    fi
    TARGET_VERSION=$(grep '^version' ../Cargo.toml | head -1 | cut -d'"' -f2)
    ;;

*)
    echo "Error: invalid selection '${METHOD}' (expected 1, 2 or 3)"
    exit 1
    ;;
esac

# ---- common deploy tail ------------------------------------------------------
if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Binary not found at ${BINARY_PATH}"
    exit 1
fi

CURRENT_VERSION=$(curl -s "https://${REMOTE_HOST}:${PORT}/version" || true)
[[ -z "$CURRENT_VERSION" ]] && CURRENT_VERSION="unknown (unreachable)"
echo ""
echo "  running:   ${CURRENT_VERSION}"
echo "  deploying: ${TARGET_VERSION:-unknown}"
echo ""
if ! yes_or_no "Proceed with deployment"; then
    exit 0
fi

echo "Stopping remote service..."
ssh ${REMOTE_USER}@${REMOTE_HOST} "sudo systemctl stop ${SERVICE_NAME}"

echo "Uploading binary..."
scp "${BINARY_PATH}" "${REMOTE_USER}@${REMOTE_HOST}:${REMOTE_PATH}/"
echo "Upload completed"

echo "Starting remote service..."
ssh ${REMOTE_USER}@${REMOTE_HOST} "sudo systemctl start ${SERVICE_NAME}"

echo "Waiting for the server to start..."
# Poll the /ping endpoint for up to (10 attempts every 2 second intervals)
for i in {1..10}; do
    response=$(curl -s "https://${REMOTE_HOST}:${PORT}/ping" || true)
    if [ "$response" == "pong" ]; then
        echo "Server is up and running!"
        exit 0
    fi
    echo "No response ($((10 - i)) attempts remaining)"
    sleep 2
done

echo "Error: Server did not respond with 'pong' on /ping endpoint."
exit 1
