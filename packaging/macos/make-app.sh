#!/bin/bash
# macOS .app + .dmg 组装（含 Applications 拖放双图标，见命名文档 checklist）。
# 用法：packaging/macos/make-app.sh [target dir, 默认 target/release]
# 要求：已 cargo build --release；签名/公证由 CI 另行处理（secrets 就绪才走）。
#
# SKIP_APP=1：跳过 .app 组装，只用现有的 target/macos-stage/RarZap.app 重打 .dmg。
#   CI 签名流程专用：必须先签名再打 DMG；若重建 .app 会把签名覆盖掉
#   （曾经的 bug：签完又跑一次本脚本，发出去的是未签名包）。
set -euo pipefail
SRC="${1:-target/release}"
VER="$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)"
STAGE="target/macos-stage"
APP="$STAGE/RarZap.app"

if [ "${SKIP_APP:-0}" != "1" ]; then
  rm -rf "$STAGE"
  mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
  cp "$SRC/rarzap" "$APP/Contents/MacOS/rarzap"
  cp assets/icons/rarzap.icns "$APP/Contents/Resources/rarzap.icns"
  cp packaging/macos/Info.plist "$APP/Contents/Info.plist"
  cp LICENSE-MIT "$APP/Contents/Resources/" 2>/dev/null || cp LICENSE-MIT "$APP/Contents/" || true

  # 版本号统一：两个版本键都覆写（只写 ShortVersion 的话 Finder/关于本机对不上）。
  set_key() {
    /usr/bin/defaults write "$PWD/$APP/Contents/Info" "$1" "$VER" 2>/dev/null || \
      sed -i '' -e "/<key>$1<\\/key>/{n;s|<string>.*</string>|<string>$VER</string>|;}" "$APP/Contents/Info.plist" || true
  }
  set_key CFBundleShortVersionString
  set_key CFBundleVersion
  chmod +x "$APP/Contents/MacOS/rarzap"
fi

if [ ! -x "$APP/Contents/MacOS/rarzap" ]; then
  echo "missing $APP (run without SKIP_APP first)" >&2
  exit 1
fi

# dmg：本应用图标 + Applications 链接并排，方便拖放安装
DMG="target/RarZap-$VER.dmg"
rm -f "$DMG"
DMGDIR="target/macos-dmg"
rm -rf "$DMGDIR"; mkdir -p "$DMGDIR"
cp -R "$APP" "$DMGDIR/"
ln -s /Applications "$DMGDIR/Applications"
hdiutil create -volname "RarZap" -srcfolder "$DMGDIR" -ov -format UDZO "$DMG"
echo "dmg: $DMG"
