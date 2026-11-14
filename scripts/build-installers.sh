#!/bin/bash
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd -P)
cd "$root"
if [ -f scripts/dev-env.sh ]; then
  source scripts/dev-env.sh
fi

case "$(uname -s)" in
  Darwin)
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
    if ! npm run tauri -- build --target universal-apple-darwin "$@"; then
      printf '\nFinder could not arrange the disk image. Building it without the custom layout.\n' >&2
      CI=true npm run tauri -- build --target universal-apple-darwin --bundles dmg "$@"
    fi
    bundle=src-tauri/target/universal-apple-darwin/release/bundle
    patterns=("dmg/*.dmg")
    ;;
  Linux)
    npm run tauri -- build "$@"
    bundle=src-tauri/target/release/bundle
    patterns=("deb/*.deb" "rpm/*.rpm" "appimage/*.AppImage")
    ;;
  MINGW* | MSYS* | CYGWIN*)
    npm run tauri -- build "$@"
    bundle=src-tauri/target/release/bundle
    patterns=("nsis/*.exe" "msi/*.msi")
    ;;
  *)
    printf 'Unsupported OS: %s\n' "$(uname -s)" >&2
    exit 1
    ;;
esac

mkdir -p release
found=0
for pattern in "${patterns[@]}"; do
  for file in "$bundle"/$pattern; do
    [ -f "$file" ] || continue
    cp -f "$file" release/
    printf 'release/%s\n' "$(basename "$file")"
    found=1
  done
done
[ "$found" -eq 1 ] || {
  printf 'No installer was produced in %s\n' "$bundle" >&2
  exit 1
}
