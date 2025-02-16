#!/bin/bash
set -euo pipefail

REMOTE_USER="pi"
REMOTE_HOST="pi"
REMOTE_PATH="/home/pi/Downloads/Rust-File-Sync_Server"
SERVICE_NAME="rust-file-sync_server.service"
TARGET="aarch64-unknown-linux-gnu"
PROJECT="server"

if [[ "${MSYSTEM-}" == "MINGW64" ]]; then
  CROSS_CMD="winpty cross build -p ${PROJECT} --release --target=${TARGET}"
else
  CROSS_CMD="cross build -p ${PROJECT} --release --target=${TARGET}"
fi

BINARY_PATH="./target/${TARGET}/release/server"
CONFIG_FILE="./target/${TARGET}/release/server.d"

echo "Building project..."
$CROSS_CMD

if [ ! -f "$BINARY_PATH" ]; then
  echo "Error: Binary not found at ${BINARY_PATH}"
  exit 1
fi

echo "Stopping remote service..."
ssh ${REMOTE_USER}@${REMOTE_HOST} "sudo systemctl stop ${SERVICE_NAME}"

echo "Uploading binary..."
scp "${BINARY_PATH}" "${REMOTE_USER}@${REMOTE_HOST}:${REMOTE_PATH}/"
echo "Uploading config file..."
scp "${CONFIG_FILE}" "${REMOTE_USER}@${REMOTE_HOST}:${REMOTE_PATH}/"

echo "Starting remote service..."
ssh ${REMOTE_USER}@${REMOTE_HOST} "sudo systemctl start ${SERVICE_NAME}"

echo "Waiting for the server to start..."
# Poll the /ping endpoint for up to 20 seconds (10 attempts with 2 second intervals)
for i in {1..10}; do
  response=$(curl -s http://${REMOTE_HOST}:3000/ping || true)
  if [ "$response" == "pong" ]; then
    echo "Server is up and running!"
    exit 0
  fi
  sleep 2
done

echo "Error: Server did not respond with 'pong' on /ping endpoint."
exit 1