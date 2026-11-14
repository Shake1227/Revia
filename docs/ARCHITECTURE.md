# アーキテクチャ

## 全体像

```
React (src/)  ──invoke──▶  Tauri コマンド (src-tauri/src/)  ──▶  git / gh（子プロセス）
     ▲                              │
     └────────── イベント ──────────┘        設定: app_config_dir/settings.json
                                            履歴: app_data_dir/history.jsonl
```

React は表示・操作・確認画面を担当し、ファイルや Git には直接触れない。Rust は入力を再検証したうえで Git / GitHub / ローカル保存を行い、結果と進行をイベントで返す。境界の型は [src/ipc/types.ts](../src/ipc/types.ts)、コマンドは [src/ipc/api.ts](../src/ipc/api.ts)、意味は [IPC仕様](API_CONTRACT.md) で定める。

## 配置

| 場所 | 内容 |
| --- | --- |
| `src/ipc/` | IPC の型、コマンド一覧、Tauri 実装、ブラウザ確認用のモック |
| `src/design/` | デザイントークン、基本スタイル、モーション定義 |
| `src/components/` | 汎用 UI 部品（ボタン、ダイアログ、メニュー、トーストなど） |
| `src/features/` | 画面と機能（一覧、詳細、操作、GitHub、履歴、設定） |
| `src/stores/` | アプリの状態（Zustand） |
| `src/i18n/` | UI 文言（日本語 `ja.ts` と英語 `en.ts`。キーで参照し、`en.ts` は `ja.ts` と同じ形を型で保証する） |
| `src-tauri/src/` | Tauri コマンド、Git / gh 実行、検出、状態判定、操作エンジン、保存 |
| `src-tauri/tests/` | 一時リポジトリを使う統合テスト |
| `src-tauri/tauri.conf.json`（Windows と Linux のウィンドウは `tauri.windows.conf.json` / `tauri.linux.conf.json` で上書き）、`src-tauri/capabilities/`、`src-tauri/icons/`、`src-tauri/Cargo.toml` の依存関係 | ウィンドウ、CSP、権限、アイコン、依存 |
| `docs/`、`scripts/` | 仕様と開発手順 |

## フロントエンド

- React 19 + TypeScript + Vite。状態は Zustand、アニメーションは Motion、アクセシブルな土台は Radix UI、コマンドパレットは cmdk、アイコンは Lucide。スタイルは CSS Modules とデザイントークン（CSS カスタムプロパティ）。
- `src/ipc/client.ts` が実行環境を見て実装を選ぶ。Tauri 内では `invoke` / `listen`、ブラウザ（`npm run dev` 単体）ではモックを使う。モックは UI の表示確認専用で、配布物の動作には関与しない。
- 画面は IPC を直接呼ばず、ストアのアクションを通す。Rust からのイベントはストアが購読して反映する。
- 文言は `src/i18n/ja.ts` に集約する。エラーは `AppError.code` から文言と対処を決める。

## バックエンド

- Git と GitHub CLI は引数配列で子プロセスとして実行する。シェルを介さず、対象パスは登録済みプロジェクトから引く。
- 書き込みはリポジトリごとに直列化し、ネットワーク操作は同時実行数を制限する。
- 認証情報は Revia では保存しない。GitHub へのログインは GitHub CLI に任せ、トークンは OS の資格情報ストアに置かれる。Git の HTTPS 認証は実行ごとに gh の credential helper を指定し、グローバルの Git 設定は変更しない。
- 設定は一時ファイル経由で置き換え、直前の正常な内容をバックアップとして残す。壊れていた場合は退避して復旧し、UI に知らせる。
- 利用者のファイルを消す操作（フォルダの削除、Git の管理をやめる）は、完全な削除を行わず OS のゴミ箱へ移す。ゴミ箱へ移す処理は差し替えられる形で持ち、テストでは一時フォルダを使う。GitHub のリポジトリの削除は `gh repo delete` に任せ、フォルダを移す前に行う（失敗したらフォルダには触れない）。
- 更新の確認だけは Git / gh を介さず、Revia 自身が GitHub の公開 API を呼ぶ（`update.rs`）。リポジトリは固定で、認証情報は使わない。取得したインストーラーは、ホストの許可リスト・大きさ・SHA-256 で検証してから開く。

## セキュリティ

- capability は `core:default` とウィンドウのドラッグだけ。Windows と Linux では OS の枠を外すので、その 2 つに限って最小化・最大化・閉じるを足す（`capabilities/window-controls.json`）。この 3 つだけはアプリのコマンドを通さず、`src/ipc/window.ts` から Tauri のウィンドウ機能を呼ぶ。macOS のメニューバーも `core:default` に含まれるメニューの権限で、フロントエンド（`src/app/useAppMenu.ts`）が組み立てる。項目の動作は画面のショートカットやコマンドパレットと同じ関数を呼ぶ。ファイル・シェル・ダイアログのプラグイン権限はフロントエンドに与えず、必要な操作は検証付きのアプリコマンドとして公開する。
- CSP は `script-src 'self'`。スタイルは UI ライブラリが `<style>` を挿入するため `'unsafe-inline'` を許可している。画像は自己ホストと GitHub のアバターのみ。
- 危険な操作（Merge、Rebase、Reset、強制 Push、削除、破棄など）は UI が影響を示して確認し、Rust 側も拒否条件を独立に検査する。強制 Push と Hard Reset は初期状態で無効。
