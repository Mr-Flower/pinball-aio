#!/bin/sh
# Crea l'AppImage a partire dal binario release. Uso: packaging/build-appimage.sh [cartella di output]
set -eu
cd "$(dirname "$0")/.."
out="${1:-.}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

appdir="$work/PinballAIO.AppDir"
install -Dm755 target/release/pinball-aio "$appdir/usr/bin/pinball-aio"
install -m755 packaging/AppRun "$appdir/AppRun"
install -m644 packaging/pinball-aio.desktop "$appdir/"
install -m644 assets/vpinball.png "$appdir/pinball-aio.png"
install -m644 assets/vpinball.png "$appdir/.DirIcon"

curl -fsSL -o "$work/appimagetool" \
  https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x "$work/appimagetool"
# --appimage-extract-and-run: funziona anche dove FUSE non c'è (runner CI, container).
ARCH=x86_64 "$work/appimagetool" --appimage-extract-and-run "$appdir" "$out/PinballAIO-x86_64.AppImage"
