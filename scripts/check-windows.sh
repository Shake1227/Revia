#!/bin/bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd -P)
source "$root/scripts/dev-env.sh"
target=x86_64-pc-windows-msvc
rustup target list --installed | grep -qx "$target" || {
  echo "先に次を実行してください: rustup target add $target" >&2
  exit 1
}
tools=$(mktemp -d "${TMPDIR:-/tmp}/revia-winrc.XXXXXXXX")
trap 'rm -rf "$tools"' EXIT
cat > "$tools/llvm-rc" <<'RC'
#!/bin/bash
out=""
prev=""
for arg in "$@"; do
  case "$prev" in /fo|-fo|/FO) out="$arg" ;; esac
  case "$arg" in /fo*|-fo*) [ "${#arg}" -gt 3 ] && out="${arg:3}" ;; esac
  prev="$arg"
done
[ -n "$out" ] && : > "$out"
exit 0
RC
chmod +x "$tools/llvm-rc"
PATH="$tools:$PATH" cargo clippy --locked --manifest-path "$root/src-tauri/Cargo.toml" --target "$target" --all-targets -- -D warnings
echo "Windows 向けの型検査: 成功"
