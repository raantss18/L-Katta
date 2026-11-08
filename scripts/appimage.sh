#!/usr/bin/env bash
# Fabrique dist/LKatta-<version>-x86_64.AppImage : un seul fichier, tout compris
# (maki, moteur d'IA intégré, choix des modèles). Aucune dépendance hors du bureau Wayland.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ARCH="${ARCH:-x86_64}"
cd "$ROOT"
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
cargo build --release
APPDIR="$ROOT/target/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp target/release/lkatta "$APPDIR/usr/bin/lkatta"
"$APPDIR/usr/bin/lkatta" icone "$APPDIR/lkatta.png" >/dev/null
cp "$APPDIR/lkatta.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/lkatta.png"
cat > "$APPDIR/lkatta.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=L Katta
Comment=Un petit lémurien assistant sur ton bureau
Exec=lkatta
Icon=lkatta
Terminal=false
Categories=Utility;
StartupWMClass=lkatta
DESK
cp "$APPDIR/lkatta.desktop" "$APPDIR/usr/share/applications/"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/lkatta" "$@"
RUN
chmod +x "$APPDIR/AppRun"
# appimagetool : variable TOOL, sinon copie déjà présente, sinon téléchargement officiel.
TOOL="${TOOL:-}"
for c in "$ROOT/target/appimagetool" "$HOME/GitHub/TONTOLO/app/build/appimagetool"; do
  [ -z "$TOOL" ] && [ -x "$c" ] && TOOL="$c"
done
if [ -z "$TOOL" ]; then
  TOOL="$ROOT/target/appimagetool"
  curl -fsSL -o "$TOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
  chmod +x "$TOOL"
fi
mkdir -p dist
OUT="$ROOT/dist/LKatta-$VERSION-$ARCH.AppImage"
ARCH="$ARCH" APPIMAGE_EXTRACT_AND_RUN=1 "$TOOL" --no-appstream "$APPDIR" "$OUT" >/dev/null
echo "$OUT"
