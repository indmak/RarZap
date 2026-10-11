#!/bin/bash
# .deb 组装：cargo build --release 产物 + 图标 + desktop 文件 -> dpkg-deb。
# 用法：packaging/linux/build-deb.sh [target dir, 默认 target/release]
set -euo pipefail
SRC="${1:-target/release}"
VER="$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)"
STAGE="target/deb-stage/rarzap_$VER"

rm -rf "target/deb-stage"
mkdir -p "$STAGE/DEBIAN" "$STAGE/usr/bin" "$STAGE/usr/share/applications" "$STAGE/usr/share/doc/rarzap"
sed "s/^Version:.*/Version: $VER/" packaging/linux/debian/DEBIAN/control > "$STAGE/DEBIAN/control"
cp "$SRC/rarzap" "$STAGE/usr/bin/rarzap"
cp packaging/linux/rarzap.desktop "$STAGE/usr/share/applications/rarzap.desktop"
cp LICENSE-MIT "$STAGE/usr/share/doc/rarzap/copyright" 2>/dev/null || cp LICENSE-MIT "$STAGE/usr/share/doc/rarzap/" || true
# hicolor 图标逐级安装
for s in 16 32 48 64 128 256 512; do
  d="$STAGE/usr/share/icons/hicolor/${s}x${s}/apps"
  mkdir -p "$d"
  cp "assets/icons/rarzap-$s.png" "$d/rarzap.png"
done
chmod 0755 "$STAGE/DEBIAN" 2>/dev/null || true
chmod 0755 "$STAGE/usr/bin/rarzap"
dpkg-deb --build "$STAGE" "target/rarzap_${VER}_amd64.deb"
echo "deb: target/rarzap_${VER}_amd64.deb"
