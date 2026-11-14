#!/usr/bin/env python3
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
rust = (root / "src-tauri/src/commands.rs").read_text()
lib = (root / "src-tauri/src/lib.rs").read_text()
ts = (root / "src/ipc/api.ts").read_text()

commands = {}
for match in re.finditer(r"#\[tauri::command\]\s*pub async fn (\w+)\s*\((.*?)\)\s*->", rust, re.S):
    args = []
    for part in re.split(r",\s*(?![^<>]*>)", match.group(2)):
        if ":" not in part:
            continue
        name, kind = (value.strip() for value in part.split(":", 1))
        if any(marker in kind for marker in ("Backend", "AppHandle", "State")):
            continue
        args.append(re.sub(r"_(\w)", lambda g: g.group(1).upper(), name))
    commands[match.group(1)] = sorted(args)

api = {}
body = ts[ts.index("export interface ReviaApi"):]
for match in re.finditer(r"\n  (\w+)\((?:args: \{(.*?)\})?\): Promise", body, re.S):
    keys, current, depth = [], "", 0
    for char in match.group(2) or "":
        depth += char in "{<("
        depth -= char in "}>)"
        if char == ";" and depth == 0:
            keys.append(current)
            current = ""
        else:
            current += char
    keys.append(current)
    snake = re.sub(r"[A-Z]", lambda g: "_" + g.group(0).lower(), match.group(1))
    api[snake] = sorted(key.split(":")[0].strip().rstrip("?") for key in keys if ":" in key)

problems = []
for name, args in api.items():
    if name not in commands:
        problems.append(f"未実装: {name}")
    elif commands[name] != args:
        problems.append(f"引数の不一致: {name} (Rust: {commands[name]} / TypeScript: {args})")
    elif f"commands::{name}" not in lib:
        problems.append(f"未登録: {name}")
problems += [f"api.ts にないコマンド: {name}" for name in commands if name not in api]

if problems:
    print("\n".join(problems), file=sys.stderr)
    sys.exit(1)
print(f"コマンドの照合: 一致（{len(api)} 件）")
