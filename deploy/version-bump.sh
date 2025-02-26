#!/bin/bash

VERSION_BUMP_BIN="./target/release/version-bump"

if [ ! -f "$VERSION_BUMP_BIN" ]; then
  echo "Building version-bump tool..."
  cargo build -p version-bump --release
fi

echo "Bumping Cargo.toml version..."
$VERSION_BUMP_BIN