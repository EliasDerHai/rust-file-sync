#!/bin/bash
# Cuts a new release: bumps the workspace version, commits, tags and pushes.
# Pushing the tag triggers .github/workflows/release.yml, which builds the
# client binary and publishes a GitHub Release. Afterwards, installed clients
# can update via update_client_linux.sh / update_client_mac.sh.
#
# Usage:
#   ./release.sh [patch|minor|major]   (default: patch)
set -euo pipefail

# repo root (script lives in deploy/)
cd "$(dirname "$0")/.."

SEMVER="${1:-patch}"
case "${SEMVER}" in
    patch|minor|major) ;;
    *) echo "Error: level must be one of patch|minor|major (got '${SEMVER}')"; exit 1 ;;
esac

# refuse to release a dirty tree (other than the version bump we're about to make)
if [[ -n "$(git status --porcelain)" ]]; then
    echo "Error: working tree is not clean. Commit or stash changes first."
    git status --short
    exit 1
fi

# ensure we're releasing what's on the remote
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
echo "Releasing from branch '${BRANCH}'..."

echo "Bumping ${SEMVER} version..."
cargo run -p version-bump -- --toml Cargo.toml --semver "${SEMVER}"

VERSION=$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
TAG="v${VERSION}"

if git rev-parse "${TAG}" >/dev/null 2>&1; then
    echo "Error: tag ${TAG} already exists."
    exit 1
fi

echo "Committing and tagging ${TAG}..."
git add Cargo.toml
git commit -m "bump version to ${VERSION}"
git tag "${TAG}"

echo "Pushing branch and tag..."
git push
git push origin "${TAG}"

echo "Release ${TAG} pushed. CI (release.yml) is now building the artifacts."

# Optionally follow the release build if gh is available.
if command -v gh >/dev/null 2>&1; then
    echo "Waiting for the release workflow..."
    RUN_ID=$(gh run list --workflow=release.yml --limit 1 --json databaseId -q '.[0].databaseId' 2>/dev/null || true)
    if [[ -n "${RUN_ID}" ]]; then
        gh run watch "${RUN_ID}" || true
    fi
    echo "Once the release is published, update installed clients with:"
    echo "  ./deploy/update_client_linux.sh"
else
    echo "Install the 'gh' CLI to auto-follow the build, or check the Actions tab."
fi
