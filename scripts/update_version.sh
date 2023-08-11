#!/bin/bash

# Extract version from tag
VERSION="$1"
# If tag has a 'v' prefix, remove it
if [[ $VERSION == v* ]]; then
    VERSION=${VERSION:1}
fi

# Update Cargo.toml with the version extracted from the tag
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
