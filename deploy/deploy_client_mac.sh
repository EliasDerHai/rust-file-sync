#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"

PROJECT="client"
SERVICE_NAME="rust-file-sync_client"
SERVICE_DIR="/Users/eliashaider/Applications/rust-file-sync_client"
EXECUTABLE_NAME="client"
PLIST_PATH="${HOME}/Library/LaunchAgents/${SERVICE_NAME}.plist"

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
mkdir -p "${HOME}/Library/LaunchAgents"

# Copy config template if no config exists
if [[ ! -f "${SERVICE_DIR}/config.yaml" ]]; then
    echo "No config found. Copying template to ${SERVICE_DIR}/config.yaml..."
    cp ../config.yaml.template "${SERVICE_DIR}/config.yaml"
    echo "IMPORTANT: Please edit ${SERVICE_DIR}/config.yaml with your settings before starting the service."
fi

# Create plist and load service on first setup
if [[ ! -f "${PLIST_PATH}" ]]; then
    echo "Creating launchd plist at ${PLIST_PATH}..."
    cat > "${PLIST_PATH}" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${SERVICE_NAME}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${SERVICE_DIR}/${EXECUTABLE_NAME}</string>
    </array>
    <key>WorkingDirectory</key>
    <string>${SERVICE_DIR}</string>
    <key>KeepAlive</key>
    <true/>
    <key>RunAtLoad</key>
    <false/>
    <key>StandardOutPath</key>
    <string>${SERVICE_DIR}/logs/${EXECUTABLE_NAME}.log</string>
    <key>StandardErrorPath</key>
    <string>${SERVICE_DIR}/logs/${EXECUTABLE_NAME}.log</string>
</dict>
</plist>
EOF
    echo "Loading launchd service..."
    launchctl load "${PLIST_PATH}"
fi

if yes_or_no "Do you want to bump the semantic version of the workspace's Cargo.toml"; then
  echo "Bumping version..."
  cargo run -p version-bump -- --toml ../Cargo.toml --semver patch
else
  echo "Skipping version bump."
fi

echo "Stopping local service via launchctl..."
launchctl stop ${SERVICE_NAME} 2>/dev/null || echo "Service was not running."

cargo build -p ${PROJECT} --release
echo "Copying binary to ${SERVICE_DIR}/${EXECUTABLE_NAME} ..."
cp ../target/release/${EXECUTABLE_NAME} ${SERVICE_DIR}/

echo "Starting local service via launchctl..."
launchctl start ${SERVICE_NAME}

# wait for service to start
sleep 1

echo "Checking service status..."
pid=$(launchctl list | grep "${SERVICE_NAME}" | awk '{print $1}' || echo "-")

if [[ "${pid}" != "-" && "${pid}" != "0" ]]; then
    echo "Service ${SERVICE_NAME} is running with PID ${pid}."
    if yes_or_no "Do you want to tail the logs?"; then
        tail -n 100 -f "${SERVICE_DIR}/logs/${EXECUTABLE_NAME}.log"
    fi
else
    echo "Error: Service ${SERVICE_NAME} failed to start."
    exit 1
fi
