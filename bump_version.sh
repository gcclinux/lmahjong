#!/bin/bash
# Usage: ./bump_version.sh 0.3.0
# Updates the version in both `release` and `Cargo.toml`, and creates a new git tag.

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <new_version>"
    echo "Example: $0 0.3.0"
    exit 1
fi

NEW_VERSION="$1"
VERSION="${NEW_VERSION#v}"
TAG="v$VERSION"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Update the release file
echo "$VERSION" > "$SCRIPT_DIR/release"

# Update Cargo.toml version field
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" "$SCRIPT_DIR/Cargo.toml"

echo "Version updated to $VERSION in both release and Cargo.toml"

# Set new git tag
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    if git rev-parse "$TAG" >/dev/null 2>&1; then
        echo "Git tag $TAG already exists."
    else
        git tag -a "$TAG" -m "Release $TAG"
        echo "Created git tag $TAG"
    fi
fi
