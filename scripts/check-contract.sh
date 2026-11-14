#!/bin/bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd -P)
source "$root/scripts/dev-env.sh"
python3 "$root/scripts/check-commands.py"
work=$(mktemp -d "${TMPDIR:-/tmp}/revia-contract.XXXXXXXX")
trap 'rm -rf "$work"' EXIT

"$root/scripts/make-test-workspace.sh" "$work/ws" > /dev/null
REVIA_CONTRACT_DUMP="$work/dump" REVIA_TEST_WORKSPACE="$work/ws" \
  cargo test --locked --manifest-path "$root/src-tauri/Cargo.toml" --test contract_dump -- --nocapture > "$work/cargo.log" 2>&1 \
  || { tail -40 "$work/cargo.log" >&2; exit 1; }

node "$root/scripts/contract-samples.mjs" "$work/dump" "$root/src/ipc/types" > "$work/samples.ts"
cat > "$work/tsconfig.json" <<JSON
{ "compilerOptions": { "strict": true, "noEmit": true, "module": "ESNext", "moduleResolution": "Bundler", "target": "ES2022", "skipLibCheck": true }, "files": ["samples.ts"] }
JSON
if "$root/node_modules/.bin/tsc" -p "$work/tsconfig.json"; then
  echo "契約チェック: 一致（$(ls "$work/dump" | wc -l | tr -d ' ') 種類の応答を検査）"
else
  echo "契約チェック: 不一致。上のエラーを確認してください。" >&2
  [ -n "${REVIA_KEEP_CONTRACT_DUMP:-}" ] && { trap - EXIT; echo "出力を残しました: $work" >&2; }
  exit 1
fi
