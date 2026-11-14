#!/bin/sh
# Puts LD ICT's icons over RustDesk's. Run from the repository root after a rebase onto a new RustDesk release, and
# commit the result. The sources in ldict/icons/ are made from ldict/icon.svg (the LD ICT logo, 1024 px PNG, a
# multi-size .ico and a macOS .icns).
set -eu
cd "$(dirname "$0")/.."
I=ldict/icons
cp "$I/1024.png" res/icon.png
cp "$I/1024.png" res/mac-icon.png
cp "$I/32.png" res/32x32.png
cp "$I/64.png" res/64x64.png
cp "$I/128.png" res/128x128.png
cp "$I/256.png" res/128x128@2x.png
cp "$I/icon.ico" res/icon.ico
cp "$I/icon.ico" res/tray-icon.ico
cp "$I/icon.ico" flutter/windows/runner/resources/app_icon.ico
cp "$I/AppIcon.icns" flutter/macos/Runner/AppIcon.icns
cp ldict/icon.svg flutter/assets/icon.svg
echo "LD ICT icons in place."
