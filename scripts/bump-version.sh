#!/bin/bash
set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <new-version>"
  echo "Example: $0 0.1.12"
  exit 1
fi

VERSION="$1"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

sed -i '' "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT/Cargo.toml"
sed -i '' "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT/editors/zed/extension.toml"
sed -i '' "s/\"version\": \"[^\"]*\"/\"version\": \"$VERSION\"/" "$ROOT/editors/code/package.json"
sed -i '' "s/^pluginVersion = .*/pluginVersion = $VERSION/" "$ROOT/editors/intellij/gradle.properties"

echo "Bumped all versions to $VERSION"
