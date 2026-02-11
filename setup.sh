#!/bin/bash
set -e

echo "=== Installing metal-analyzer ==="
cargo install --path crates/metal-analyzer
echo ""
metal-analyzer --version
echo ""
echo "=== Zed Extension ==="
echo "1. Open Zed"
echo "2. Cmd+Shift+P → 'zed: install dev extension'"
echo "3. Select the 'editors/zed' folder"
