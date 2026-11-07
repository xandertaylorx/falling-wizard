#!/usr/bin/env bash
# Falling Wizard installer for macOS.
# Builds the game from source (no downloads or releases needed) and installs
# "Falling Wizard.app" into ~/Applications plus a `falling-wizard` command.
#
#   curl -fsSL https://raw.githubusercontent.com/xandertaylorx/falling-wizard/main/web/install.sh | bash
set -euo pipefail

REPO="https://github.com/xandertaylorx/falling-wizard.git"
DIR="$HOME/.falling-wizard"
APP="$HOME/Applications/Falling Wizard.app"
BIN="$HOME/.local/bin"

say() { printf "\033[1;35m*\033[0m %s\n" "$1"; }

if [ "$(uname)" != "Darwin" ]; then
  echo "This installer is for macOS."
  exit 1
fi

if ! xcode-select -p >/dev/null 2>&1; then
  say "Installing Apple's command line tools (a window will pop up)..."
  xcode-select --install || true
  echo "Run this command again once the tools have finished installing."
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  if [ -x "$HOME/.cargo/bin/cargo" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  else
    say "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal >/dev/null
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
fi

if [ -d "$DIR/src/.git" ]; then
  say "Updating Falling Wizard..."
  git -C "$DIR/src" pull --ff-only --quiet
else
  say "Fetching Falling Wizard..."
  mkdir -p "$DIR"
  git clone --depth 1 --quiet "$REPO" "$DIR/src"
fi

say "Building (a couple of minutes the first time)..."
cargo build --release --quiet --manifest-path "$DIR/src/Cargo.toml"

say "Installing..."
mkdir -p "$APP/Contents/MacOS" "$BIN"
# Copy then rename, so a running copy of the game isn't disturbed mid-update.
cp "$DIR/src/target/release/falling-wizard" "$APP/Contents/MacOS/falling-wizard.new"
mv -f "$APP/Contents/MacOS/falling-wizard.new" "$APP/Contents/MacOS/falling-wizard"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Falling Wizard</string>
  <key>CFBundleDisplayName</key><string>Falling Wizard</string>
  <key>CFBundleIdentifier</key><string>com.fallingwizard.game</string>
  <key>CFBundleExecutable</key><string>falling-wizard</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
ln -sf "$APP/Contents/MacOS/falling-wizard" "$BIN/falling-wizard"

say "Installed! Open \"Falling Wizard\" from ~/Applications or Launchpad, or run: falling-wizard"
case ":$PATH:" in
  *":$BIN:"*) ;;
  *) echo "  (add $BIN to your PATH to use the falling-wizard command)" ;;
esac
say "Run the same command again any time to update (or use the button in the game)."
[ -n "${FW_NO_OPEN:-}" ] || open "$APP"
