#!/bin/sh
# Builds the macOS-only helpers: Apple Vision (fg-vision) and the ImageIO image helper (fg-image). Other platforms simply run without them.
set -e
cd "$(dirname "$0")/../src-tauri"
mkdir -p bin
swiftc -O -target "$(uname -m)-apple-macos15.0" vision/fg-vision.swift -o bin/fg-vision
strip -x bin/fg-vision   # no symbol names in the helper
echo "built src-tauri/bin/fg-vision"
swiftc -O -target "$(uname -m)-apple-macos15.0" vision/fg-image.swift -o bin/fg-image
strip -x bin/fg-image
echo "built src-tauri/bin/fg-image"
