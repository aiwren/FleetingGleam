#!/bin/sh
# Builds the macOS-only Apple Vision helper. Other platforms simply run without it.
set -e
cd "$(dirname "$0")/../src-tauri"
mkdir -p bin
swiftc -O -target "$(uname -m)-apple-macos15.0" vision/fg-vision.swift -o bin/fg-vision
strip -x bin/fg-vision   # no symbol names in the helper
echo "built src-tauri/bin/fg-vision"
