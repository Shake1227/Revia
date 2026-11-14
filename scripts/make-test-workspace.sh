#!/bin/bash
set -euo pipefail
ws="$1"
rm -rf "$ws"
mkdir -p "$ws/remotes" "$ws/projects/group"
export GIT_CONFIG_GLOBAL="$ws/gitconfig" GIT_CONFIG_NOSYSTEM=1
git config --file "$ws/gitconfig" user.name "Revia Test"
git config --file "$ws/gitconfig" user.email "test@example.invalid"
git config --file "$ws/gitconfig" init.defaultBranch main
git config --file "$ws/gitconfig" commit.gpgsign false

mk() {
  git init -q --bare "$ws/remotes/$1.git"
  git clone -q "$ws/remotes/$1.git" "$ws/projects/$1" 2>/dev/null
  ( cd "$ws/projects/$1" && git config user.name "Revia Test" && git config user.email "test@example.invalid" && git config commit.gpgsign false \
    && printf '# %s\n' "$1" > README.md && printf 'export const value = 1;\n' > index.ts && git add -A && git commit -q -m "最初のコミット" && git push -q -u origin main )
}
other() {
  rm -rf "$ws/tmp-$1" && git clone -q "$ws/remotes/$1.git" "$ws/tmp-$1"
  ( cd "$ws/tmp-$1" && git config user.name "Other Dev" && git config user.email "other@example.invalid" && git config commit.gpgsign false \
    && printf '%s\n' "$2" >> "$3" && git add -A && git commit -q -m "$4" && git push -q )
  rm -rf "$ws/tmp-$1"
}

mk synced-app
mk dirty-notes
( cd "$ws/projects/dirty-notes" && printf 'export const value = 2;\n' > index.ts && printf '下書き\n' > "メモ.md" )
mk ahead-api
( cd "$ws/projects/ahead-api" && printf 'export const extra = true;\n' > extra.ts && git add -A && git commit -q -m "extra を追加" && printf 'more\n' >> README.md && git commit -qam "README を更新" )
mk behind-ui
other behind-ui "リモートの変更" README.md "リモートで README を更新"
( cd "$ws/projects/behind-ui" && git fetch -q )
mk diverged-cli
other diverged-cli "export const value = 99;" index.ts "リモートで値を変更"
( cd "$ws/projects/diverged-cli" && printf 'export const value = 42;\n' > index.ts && git commit -qam "ローカルで値を変更" && git fetch -q )
mk feature-work
( cd "$ws/projects/feature-work" && git switch -q -c feature/login && printf 'login\n' > login.ts && git add -A && git commit -q -m "ログイン画面を追加" && git switch -q main && git switch -q -c feature/new-ui && printf 'ui\n' > ui.ts && git add -A && git commit -q -m "新しい UI" )
mkdir -p "$ws/projects/group/local-only" && ( cd "$ws/projects/group/local-only" && git init -q && git config user.name "Revia Test" && git config user.email "test@example.invalid" && git config commit.gpgsign false && printf 'x\n' > a.txt && git add -A && git commit -q -m "初期" )
mkdir -p "$ws/projects/group/plain-site" && printf '{"name":"plain-site"}\n' > "$ws/projects/group/plain-site/package.json" && printf '<h1>site</h1>\n' > "$ws/projects/group/plain-site/index.html"
mkdir -p "$ws/projects/not-a-project" && printf 'just a file\n' > "$ws/projects/not-a-project/notes.txt"
echo "workspace ready: $ws/projects"
