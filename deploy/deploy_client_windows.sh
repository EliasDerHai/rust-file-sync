#!/bin/bash
set -euo pipefail

PROJECT="client"
SERVICE_NAME="rust-file-sync_client"
SERVICE_PATH="C:/projects/code/backend/rust-file-sync_windows_service/client.exe"

echo "Stopping local service via nssm..."
nssm stop ${SERVICE_NAME};

status=$(nssm status ${SERVICE_NAME})
if [ ! "$status" == "SERVICE_STOPPED" ]; then
  echo "nssm couldn't stop ${SERVICE_NAME} - still ${status}"
  exit 1
fi

cargo build -p ${PROJECT} --release
echo "Copying binary to ${SERVICE_PATH} ..."
cp ../target/release/client.exe ${SERVICE_PATH}

echo "Starting local service via nssm..."
nssm start ${SERVICE_NAME};

for i in {1..10}; do
  status=$(nssm status ${SERVICE_NAME})
  if [ "$status" == "SERVICE_RUNNING" ]; then
    exit 0
  fi
  echo "nssm couldn't start ${SERVICE_NAME} - status is ${status} ($((10 - i)) attempts remaining)"
  sleep 2
done

echo "Error: giving up - you might wanna check ./logs/stdout.txt"
exit 1
