#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAP_SKELETON_DIR="$ROOT_DIR/packaging/homebrew/tap-skeleton"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

VERSION="${VERSION:-$(node -p "require('$ROOT_DIR/package.json').version")}"
TAG_NAME="${TAG_NAME:-v$VERSION}"
DMG_PATH="${DMG_PATH:-}"
HOMEBREW_TAP_TOKEN="${HOMEBREW_TAP_TOKEN:-}"
HOMEBREW_TAP_REPOSITORY="${HOMEBREW_TAP_REPOSITORY:-${GITHUB_REPOSITORY_OWNER:-faker6996}/homebrew-tap}"
HOMEBREW_TAP_BRANCH="${HOMEBREW_TAP_BRANCH:-main}"
MACOS_REQUIREMENT="${MACOS_REQUIREMENT:->= :ventura}"
RELEASE_OWNER_REPO="${RELEASE_OWNER_REPO:-${GITHUB_REPOSITORY:-faker6996/record-screen}}"

if [[ -z "$DMG_PATH" ]]; then
  cat <<'EOF' >&2
Missing DMG_PATH.

Usage:
  HOMEBREW_TAP_TOKEN=<token> \
  DMG_PATH=/path/to/Record.Screen_0.1.0_universal.dmg \
  scripts/publish-homebrew-tap.sh

Optional:
  HOMEBREW_TAP_REPOSITORY=faker6996/homebrew-tap
  HOMEBREW_TAP_BRANCH=main
  TAG_NAME=v0.1.0
  VERSION=0.1.0
  RELEASE_OWNER_REPO=faker6996/record-screen
  MACOS_REQUIREMENT=">= :ventura"
EOF
  exit 1
fi

if [[ ! -f "$DMG_PATH" ]]; then
  echo "DMG not found: $DMG_PATH" >&2
  exit 1
fi

if [[ -z "$HOMEBREW_TAP_TOKEN" ]]; then
  echo "HOMEBREW_TAP_TOKEN is required." >&2
  exit 1
fi

DMG_FILENAME="$(basename "$DMG_PATH")"
SHA256="$(shasum -a 256 "$DMG_PATH" | awk '{print $1}')"
URL="https://github.com/$RELEASE_OWNER_REPO/releases/download/$TAG_NAME/$DMG_FILENAME"

TAP_DIR="$WORK_DIR/homebrew-tap"
git clone \
  --depth 1 \
  --branch "$HOMEBREW_TAP_BRANCH" \
  "https://x-access-token:${HOMEBREW_TAP_TOKEN}@github.com/${HOMEBREW_TAP_REPOSITORY}.git" \
  "$TAP_DIR"

mkdir -p "$TAP_DIR/Casks" "$TAP_DIR/.github/workflows"

VERSION="$VERSION" \
SHA256="$SHA256" \
URL="$URL" \
MACOS_REQUIREMENT="$MACOS_REQUIREMENT" \
  "$ROOT_DIR/scripts/render-homebrew-cask.sh" "$TAP_DIR/Casks/record-screen.rb"

if [[ ! -f "$TAP_DIR/README.md" ]]; then
  cp "$TAP_SKELETON_DIR/README.md" "$TAP_DIR/README.md"
fi

if [[ ! -f "$TAP_DIR/.github/workflows/validate-cask.yml" ]]; then
  cp \
    "$TAP_SKELETON_DIR/.github/workflows/validate-cask.yml.template" \
    "$TAP_DIR/.github/workflows/validate-cask.yml"
fi

pushd "$TAP_DIR" >/dev/null

if git diff --quiet -- Casks/record-screen.rb README.md .github/workflows/validate-cask.yml; then
  echo "Homebrew tap is already up to date."
  exit 0
fi

git config user.name "${GIT_AUTHOR_NAME:-github-actions[bot]}"
git config user.email "${GIT_AUTHOR_EMAIL:-41898282+github-actions[bot]@users.noreply.github.com}"
git add Casks/record-screen.rb README.md .github/workflows/validate-cask.yml
git commit -m "Update record-screen cask to ${TAG_NAME}"
git push origin "$HOMEBREW_TAP_BRANCH"

popd >/dev/null

echo "Published Homebrew cask to ${HOMEBREW_TAP_REPOSITORY}@${HOMEBREW_TAP_BRANCH}"
