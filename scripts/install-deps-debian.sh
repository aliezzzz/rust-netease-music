#!/usr/bin/env bash
set -euo pipefail
sudo apt-get update
driver=webkit2gtk-driver
if apt-cache show webkitgtk-webdriver >/dev/null 2>&1; then driver=webkitgtk-webdriver; fi
sudo apt-get install -y --no-install-recommends build-essential pkg-config dbus-x11 patchelf python3-gi \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libssl-dev libcurl4-openssl-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  libgstreamer-plugins-bad1.0-dev gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly \
  gstreamer1.0-libav xvfb "$driver" playerctl
