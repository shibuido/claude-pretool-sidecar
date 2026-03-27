#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Read version from Cargo.toml
VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
PACKAGE_NAME="claude-pretool-sidecar"
ARCH="$(uname -m)"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCHIVE_NAME="${PACKAGE_NAME}-${VERSION}-${OS}-${ARCH}"
OUT_DIR="$PROJECT_DIR/target/release-archives"

echo "=== Building ${PACKAGE_NAME} v${VERSION} for ${OS}-${ARCH} ==="

# Build release binaries
cd "$PROJECT_DIR"
cargo build --release

# Create archive directory
mkdir -p "$OUT_DIR/$ARCHIVE_NAME"

# Copy binaries
for bin in claude-pretool-sidecar claude-pretool-logger claude-pretool-analyzer; do
    cp "target/release/$bin" "$OUT_DIR/$ARCHIVE_NAME/"
done

# Copy supporting files
cp LICENSE "$OUT_DIR/$ARCHIVE_NAME/"
cp README.md "$OUT_DIR/$ARCHIVE_NAME/"

# Create tar.gz archive
cd "$OUT_DIR"
tar czf "${ARCHIVE_NAME}.tar.gz" "$ARCHIVE_NAME"
rm -rf "$ARCHIVE_NAME"

echo ""
echo "=== Archive created ==="
echo "  $OUT_DIR/${ARCHIVE_NAME}.tar.gz"
echo ""
echo "=== Next steps ==="
echo "  1. Tag the release:"
echo "     git tag -a v${VERSION} -m 'Release v${VERSION}'"
echo "     git push origin v${VERSION}"
echo ""
echo "  2. Create a GitHub release:"
echo "     gh release create v${VERSION} \\"
echo "       '$OUT_DIR/${ARCHIVE_NAME}.tar.gz' \\"
echo "       --title 'v${VERSION}' \\"
echo "       --notes 'Release v${VERSION}'"
echo ""
echo "  3. Update the Homebrew formula sha256 with:"
echo "     shasum -a 256 '$OUT_DIR/${ARCHIVE_NAME}.tar.gz'"
echo ""
echo "  4. Publish to crates.io:"
echo "     cargo publish"
