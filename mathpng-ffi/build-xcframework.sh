#!/bin/bash
# Build script for MathRenderer.xcframework
# Builds the Rust library for all Apple platforms and generates Swift bindings

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== Building MathRenderer XCFramework ==="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check for required tools
if ! command -v rustup &> /dev/null; then
    echo -e "${RED}Error: rustup not found. Install Rust first.${NC}"
    exit 1
fi

if ! command -v xcodebuild &> /dev/null; then
    echo -e "${RED}Error: xcodebuild not found. Install Xcode first.${NC}"
    exit 1
fi

# Install required targets
echo -e "${YELLOW}Installing Rust targets...${NC}"
rustup target add aarch64-apple-darwin      2>/dev/null || true  # macOS ARM
rustup target add x86_64-apple-darwin       2>/dev/null || true  # macOS Intel
rustup target add aarch64-apple-ios         2>/dev/null || true  # iOS device
rustup target add aarch64-apple-ios-sim     2>/dev/null || true  # iOS simulator

# Build all targets
echo -e "${YELLOW}Building for all targets...${NC}"

echo "  - Building aarch64-apple-darwin (macOS ARM)..."
cargo build --release --target aarch64-apple-darwin

echo "  - Building x86_64-apple-darwin (macOS Intel)..."
cargo build --release --target x86_64-apple-darwin

echo "  - Building aarch64-apple-ios (iOS device)..."
cargo build --release --target aarch64-apple-ios

echo "  - Building aarch64-apple-ios-sim (iOS simulator)..."
cargo build --release --target aarch64-apple-ios-sim

# Create output directory
mkdir -p generated

# Generate Swift bindings using the macOS ARM library (any platform works)
echo -e "${YELLOW}Generating Swift bindings...${NC}"
cargo run --bin uniffi-bindgen generate \
    --library target/aarch64-apple-darwin/release/libmathpng_ffi.dylib \
    --language swift \
    --out-dir ./generated

# Create fat library for macOS (ARM + Intel)
echo -e "${YELLOW}Creating macOS universal library...${NC}"
mkdir -p target/universal-macos
lipo -create \
    target/aarch64-apple-darwin/release/libmathpng_ffi.a \
    target/x86_64-apple-darwin/release/libmathpng_ffi.a \
    -output target/universal-macos/libmathpng_ffi.a

# Create headers with module-specific subdirectory to avoid modulemap collision
rm -rf target/headers
mkdir -p target/headers/mathpng_ffiFFI
cp generated/mathpng_ffiFFI.h target/headers/mathpng_ffiFFI/

# Create module.modulemap inside the subdirectory
cat > target/headers/mathpng_ffiFFI/module.modulemap << 'EOF'
module mathpng_ffiFFI {
    header "mathpng_ffiFFI.h"
    export *
}
EOF

# Clean up any existing XCFramework
rm -rf MathRenderer.xcframework

# Create XCFramework
echo -e "${YELLOW}Creating XCFramework...${NC}"
xcodebuild -create-xcframework \
    -library target/universal-macos/libmathpng_ffi.a \
    -headers target/headers \
    -library target/aarch64-apple-ios/release/libmathpng_ffi.a \
    -headers target/headers \
    -library target/aarch64-apple-ios-sim/release/libmathpng_ffi.a \
    -headers target/headers \
    -output MathRenderer.xcframework

# Verify the framework was created
if [ -d "MathRenderer.xcframework" ]; then
    echo -e "${GREEN}=== Build Success! ===${NC}"

    # Copy xcframework to Xcode project
    echo -e "${YELLOW}Copying xcframework to Xcode project...${NC}"
    rm -rf ../../mitosu/Mitosu/MathRenderer.xcframework
    cp -R MathRenderer.xcframework ../../mitosu/Mitosu/

    # Copy Swift bindings to Xcode project
    echo -e "${YELLOW}Copying Swift bindings to Xcode project...${NC}"
    cp generated/mathpng_ffi.swift ../../mitosu/Mitosu/Mitosu/Utilities/

    echo -e "${GREEN}=== All Done! ===${NC}"
    echo ""
    echo "Generated and copied:"
    echo "  - MathRenderer.xcframework -> ../../mitosu/Mitosu/MathRenderer.xcframework"
    echo "  - mathpng_ffi.swift -> ../../mitosu/Mitosu/Mitosu/Utilities/mathpng_ffi.swift"
    echo ""
    ls -la MathRenderer.xcframework/
else
    echo -e "${RED}Error: XCFramework was not created${NC}"
    exit 1
fi
