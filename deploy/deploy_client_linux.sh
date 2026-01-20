#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

PROJECT="client"
SERVICE_NAME="rust-file-sync-client"
SERVICE_PATH="${HOME}/.local/bin"
EXECUTABLE_NAME="client"
CONFIG_DIR="${HOME}/.config/rust-file-sync"
LOG_DIR="${HOME}/.local/share/rust-file-sync/logs"
SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
SERVICE_FILE="${SYSTEMD_USER_DIR}/${SERVICE_NAME}.service"

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
mkdir -p "${SERVICE_PATH}"
mkdir -p "${CONFIG_DIR}"
mkdir -p "${LOG_DIR}"
mkdir -p "${SYSTEMD_USER_DIR}"

# Copy config template if no config exists
if [[ ! -f "${CONFIG_DIR}/config.yaml" ]]; then
    echo "No config found. Copying template to ${CONFIG_DIR}/config.yaml..."
    cp ../config.yaml.template "${CONFIG_DIR}/config.yaml"
    echo "IMPORTANT: Please edit ${CONFIG_DIR}/config.yaml with your settings before starting the service."
fi

# Create or update systemd service file
if [[ ! -f "${SERVICE_FILE}" ]] || ! grep -q "WorkingDirectory" "${SERVICE_FILE}"; then
    echo "Creating/updating systemd user service file at ${SERVICE_FILE}..."
    cat > "${SERVICE_FILE}" << EOF
[Unit]
Description=Rust File Sync Client
After=network.target

[Service]
ExecStart=${SERVICE_PATH}/${EXECUTABLE_NAME}
WorkingDirectory=${CONFIG_DIR}
Restart=on-failure
RestartSec=5
Environment="RUST_LOG=info"

[Install]
WantedBy=default.target
EOF
    echo "Reloading systemd user daemon..."
    systemctl --user daemon-reload
    echo "Enabling service to start on login..."
    systemctl --user enable ${SERVICE_NAME}
fi

if yes_or_no "Do you want to bump the semantic version of the workspace's Cargo.toml"; then
  echo "Bumping version..."
  cargo run -p version-bump -- --toml ../Cargo.toml --semver patch
else
  echo "Skipping version bump."
fi

echo "Stopping service via systemctl --user..."
systemctl --user stop ${SERVICE_NAME} 2>/dev/null || echo "Service was not running."

cargo build -p ${PROJECT} --release
echo "Copying binary to ${SERVICE_PATH} ..."
cp ../target/release/${EXECUTABLE_NAME} ${SERVICE_PATH}/

echo "Starting service via systemctl --user..."
systemctl --user start ${SERVICE_NAME}

# wait for service to start
sleep 1

echo "Checking service status..."
if systemctl --user is-active --quiet ${SERVICE_NAME}; then
    echo "Service ${SERVICE_NAME} is running."
    systemctl --user status ${SERVICE_NAME} --no-pager
    if yes_or_no "Do you want to tail the logs?"; then
        journalctl --user -u ${SERVICE_NAME} -f -n 100
    fi
else
    echo "Error: Service ${SERVICE_NAME} failed to start."
    systemctl --user status ${SERVICE_NAME} --no-pager || true
    exit 1
fi
