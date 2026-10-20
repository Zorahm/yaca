#!/usr/bin/env bash
# Puts Yaca's launcher icon into the Android project `tauri android init` made.
#
#   npm run tauri android init     # once
#   scripts/android-icons.sh       # after init, and whenever the icon changes
#   npm run tauri android build -- --apk ...
#
# `init` writes the stock Tauri icon. This copies the adaptive icon over it
# (src-tauri/icons/android, made by scripts/android_icon.py) and makes sure the
# manifest names it, round variant included. The adaptive icon has a monochrome
# layer, which is what lets Android 13 and later recolour it to the theme.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
project="${ANDROID_PROJECT:-$root/src-tauri/gen/android}"
res="$project/app/src/main/res"
manifest="$project/app/src/main/AndroidManifest.xml"

[ -d "$res" ] || { echo "error: $res not found; run 'npm run tauri android init' first" >&2; exit 1; }
[ -f "$manifest" ] || { echo "error: $manifest not found" >&2; exit 1; }

cp -r "$root/src-tauri/icons/android/." "$res/"

grep -q 'android:icon="@mipmap/ic_launcher"' "$manifest" ||
  { echo "error: the manifest does not set android:icon; the template changed" >&2; exit 1; }
if ! grep -q 'android:roundIcon=' "$manifest"; then
  sed -i 's|android:icon="@mipmap/ic_launcher"|android:icon="@mipmap/ic_launcher"\n        android:roundIcon="@mipmap/ic_launcher_round"|' "$manifest"
fi

echo "launcher icon installed in $project"
