#!/bin/sh
# Spine for Linux, in one line:
#   curl -fsSL https://raw.githubusercontent.com/paperwolf2350-lab/spine/main/install.sh | sh
# Downloads the latest Spine AppImage to ~/Applications, checks its fingerprint,
# and adds Spine to your app menu. Nothing needs root. Run it again any time;
# Spine also updates itself from Settings -> Updates.
set -eu

REPO="paperwolf2350-lab/spine"
BASE="https://github.com/$REPO/releases/latest/download"
DEST="${HOME}/Applications"
APP="$DEST/Spine-x86_64.AppImage"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"

say() { printf '%s\n' "$*"; }

if [ "$(uname -m)" != "x86_64" ]; then
  say "Spine for Linux needs a 64-bit Intel/AMD PC (this one is $(uname -m))."
  exit 1
fi

fetch() {
  if command -v curl >/dev/null 2>&1; then curl -fL --progress-bar -o "$2" "$1"
  elif command -v wget >/dev/null 2>&1; then wget -q --show-progress -O "$2" "$1"
  else say "Please install curl or wget first."; exit 1; fi
}

mkdir -p "$DEST"
TMP="$DEST/.Spine-download.AppImage"
say "Downloading Spine..."
fetch "$BASE/Spine-x86_64.AppImage" "$TMP"
fetch "$BASE/Spine-x86_64.AppImage.sha256" "$TMP.sha256"

WANT=$(cut -d' ' -f1 < "$TMP.sha256")
HAVE=$(sha256sum "$TMP" | cut -d' ' -f1)
rm -f "$TMP.sha256"
if [ "$WANT" != "$HAVE" ]; then
  rm -f "$TMP"
  say "The download didn't match its fingerprint, so it wasn't installed. Please try again."
  exit 1
fi
chmod +x "$TMP"
mv -f "$TMP" "$APP"

# Icon and app menu entry (Spine keeps them up to date itself after the first start).
mkdir -p "$DATA/icons/hicolor/512x512/apps"
fetch "https://raw.githubusercontent.com/$REPO/main/spine-app/icons/icon.png" "$DATA/icons/hicolor/512x512/apps/spine.png" 2>/dev/null || true
mkdir -p "$DATA/applications"
cat > "$DATA/applications/spine.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Spine
GenericName=Game Library
Comment=Your game library, like a console
Exec="$APP"
Icon=spine
Terminal=false
Categories=Game;Emulator;
StartupWMClass=Spine
DESKTOP
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DATA/applications" >/dev/null 2>&1 || true

say ""
say "Spine is installed. Open it from your app menu, or run: $APP"
