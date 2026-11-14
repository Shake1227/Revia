#!/usr/bin/env python3
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "src-tauri"
SHARED = ["label", "title", "width", "height", "minWidth", "minHeight", "backgroundColor"]

def load(name):
    return json.loads((ROOT / name).read_text(encoding="utf-8"))

def window(name):
    windows = load(name)["app"]["windows"]
    if len(windows) != 1:
        raise SystemExit(f"{name}: ウィンドウは 1 つだけのはずです（{len(windows)} 個ありました）")
    return windows[0]

def main():
    problems = []
    base = window("tauri.conf.json")
    if base.get("titleBarStyle") != "Overlay" or base.get("decorations") is False:
        problems.append("tauri.conf.json: macOS は OS の信号機ボタンを使う（titleBarStyle が Overlay、decorations は外さない）")
    for name in ["tauri.windows.conf.json", "tauri.linux.conf.json"]:
        other = window(name)
        for key in SHARED:
            if other.get(key) != base.get(key):
                problems.append(f"{name}: {key} が tauri.conf.json と違います（{other.get(key)!r} / {base.get(key)!r}）")
        if other.get("decorations") is not False:
            problems.append(f"{name}: decorations は false（Revia がウィンドウの操作ボタンを描く）")

    capability = load("capabilities/window-controls.json")
    if sorted(capability.get("platforms", [])) != ["linux", "windows"]:
        problems.append("capabilities/window-controls.json: platforms は windows と linux だけ")
    for permission in ["core:window:allow-minimize", "core:window:allow-toggle-maximize", "core:window:allow-close"]:
        if permission not in capability.get("permissions", []):
            problems.append(f"capabilities/window-controls.json: {permission} がありません")
    if capability["identifier"] not in load("tauri.conf.json")["app"]["security"]["capabilities"]:
        problems.append("tauri.conf.json: app.security.capabilities に window-controls がありません")

    if problems:
        print("\n".join(problems), file=sys.stderr)
        raise SystemExit(1)
    print("ウィンドウの設定: 3 つのファイルがそろっています")

if __name__ == "__main__":
    main()
