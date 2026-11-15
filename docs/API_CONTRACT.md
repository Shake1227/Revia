# IPC仕様

React（`src/`）と Rust（`src-tauri/`）の境界を定める。型は [src/ipc/types.ts](../src/ipc/types.ts)、コマンド一覧は [src/ipc/api.ts](../src/ipc/api.ts) が正で、この文書は意味・副作用・安全上の決まりを定める。型と実装が食い違っていたら、型のほうを正として実装を直す。

## 1. 共通規約

- React から Tauri の `invoke` を呼ぶ。コマンド名は `snake_case`、引数と JSON フィールドは `camelCase`、列挙値は `snake_case`。判別共用体のタグは `kind`（`PlanStep` と `PlanWarning` は `code`）。
- `api.ts` のメソッド名 `camelCase` がコマンド名 `snake_case` に 1 対 1 で対応する。引数オブジェクトのキーがそのままコマンド引数になる。
- 日時はすべて Unix 時刻のミリ秒（number）。
- `T | null` は必ずキーを含めて `null` を返す。キーを省略しない。入力側の `?` 付きフィールドは省略と `null` を同じに扱う。
- 失敗時は `invoke` を拒否し、`AppError { code, message, detail }` を返す。`message` は短い英語の診断文、`detail` は秘密情報を除去した Git / gh の出力、またはバックエンドが英語で書いた補足（なければ `null`）。UI は日本語と英語を切り替えるため、画面の文言と対処は `code` から決める。`message` は画面に出さず、`detail` は「詳細を表示」の中でそのまま見せる。バックエンドは利用者向けの文を特定の言語で組み立てない（伏せ字の印は `[redacted]`）。
- Rust はすべての入力を再検証する。プロジェクトは `projectId` で指定し、パスは登録情報から引く。フロントエンドから受け取った絶対パスで Git を実行しない（例外は `add_scan_root`、`add_project`、`restore_excluded_path`、`clone` の保存先で、いずれも正規化して存在と種類を確認する）。
- トークン、認証ヘッダー、資格情報入りの URL、環境変数を、レスポンス・イベント・履歴・ログに含めない。URL の userinfo（`https://user:token@host/…`）は、リモート URL・エラーの `detail`・履歴から必ず取り除く。
- 秘密らしい行（`token=`、`Authorization:` など）を丸ごと伏せる処理は、Git / gh の診断出力（エラーの `detail`）と履歴だけに適用する。差分の本文、コミットメッセージ、作者名、ファイルパスには適用しない（利用者自身の内容であり、コミット前に秘密の混入へ気付けなくなるため）。
- 時間のかかる処理は非同期で実行し、UI スレッドを止めない。同じリポジトリへの書き込みは直列化する。

### イベント

`main` ウィンドウへ `emit` する。イベント名と payload は `ReviaEvents` を参照。

| イベント | 送るとき |
| --- | --- |
| `revia:project-updated` | 1 件のプロジェクトの状態か設定が変わった（操作後・更新後）。変化がなければ送らない |
| `revia:projects-changed` | 一覧の構成が変わった（スキャン、追加、除外、Clone）。全件を送る |
| `revia:operation` | 操作の項目が変化した（`item`）、操作全体の状態が変わった（`state`） |
| `revia:scan-progress` | スキャン中（200ms 以上の間隔）と各ルートの完了時 |
| `revia:github-login` | ログインの進行 |
| `revia:update-progress` | 更新のダウンロード中（150ms 以上の間隔）と完了時 |

## 2. 永続化

- 設定は Tauri の `app_config_dir()` 直下の `settings.json` に保存する。保存内容は `AppSettings`、プロジェクトごとの `ProjectSettings`（キーは正規化したパス）、手動追加したプロジェクトのパス、`schemaVersion`。
- 書き込みは同じディレクトリの一時ファイルへ書いて `fsync` し、`rename` で置き換える。置き換える前の正常な内容を `settings.json.bak` に残す。
- 読み込みに失敗（JSON が壊れている、型が合わない）した場合は、壊れたファイルを `settings.corrupt-<時刻>.json` へ退避し、`settings.json.bak` が読めればそれを採用（`recovery.kind = "restored_backup"`）、読めなければ初期値（`"reset"`）。黙って初期化しない。`get_app_state` の `recovery` で UI に伝える（一度返したらクリアしてよい）。
- 未知のフィールドは無視し、欠けているフィールドは既定値で補う。将来のバージョンで追加したフィールドを古い版が消さないよう、`ui` は中身を解釈せずオブジェクトのまま保持する。
- 操作履歴は `app_data_dir()` の `history.jsonl`。最大 2,000 件で、超えたら古いものから削除する。`params` には、分かる範囲で `branch`（操作時のブランチ）、`target`（`origin/main` 形式の操作先）、`pulled` / `pushed`（コミット数）、`files`、`commits`、`source`、`remote`、`repo`、`mode`、`sha`、`name` を入れる。
- 認証情報は Revia では保存しない（§7）。
- Revia が保存するファイル（`settings.json` とその `.bak`、キャッシュの `projects.json`、`history.jsonl`、複製した背景画像）は、Unix では利用者だけが読み書きできる権限で作る（ファイル 0600、Revia が作るディレクトリ 0700）。既にあるファイルも、次に書き込むときに直す。パスやリモートの URL、非公開リポジトリの名前が入るため。Windows は既定の ACL のままにする。
- 既定値: `general.autoRefresh = { onFocus: true, intervalMinutes: 10, fetch: false }`、`concurrency = 4`、`editor = null`、`safety = { allowForcePush: false, allowHardReset: false }`、`sync = { commit: true, commitMessage: "", diverged: "stop" }`（`diverged` は `"stop"` / `"merge"` / `"prefer_local"` / `"prefer_remote"`、`commitMessage` は 200 文字まで）。`ProjectSettings` は `{ favorite: false, includeInBatch: true, defaultRemote: null, defaultBranch: null, pullMode: "ff_only", branchTargets: {} }`。

## 3. プロジェクトの検出

`ScanRoot` ごとに `maxDepth`（既定 4、1〜8）まで探索する。

- ディレクトリに `.git`（ディレクトリまたはファイル）があれば Git プロジェクト。その下は探索しない。
- `.git` がなく、次のいずれかがあれば Git 未初期化のプロジェクト。その下は探索しない: `package.json`、`Cargo.toml`、`pyproject.toml`、`requirements.txt`、`go.mod`、`pom.xml`、`build.gradle`、`build.gradle.kts`、`Package.swift`、`*.xcodeproj`、`*.xcworkspace`、`Gemfile`、`composer.json`、`pubspec.yaml`、`CMakeLists.txt`、`Makefile`、`deno.json`、`*.sln`、`*.csproj`、`mix.exs`。
- 探索しない: 隠しディレクトリ（`.` で始まる）、`node_modules`、`target`、`dist`、`build`、`out`、`vendor`、`Pods`、`venv`、`__pycache__`、`Library`、`Applications`、`.Trash`、シンボリックリンク、読み取れないディレクトリ（エラーにせず飛ばす）。
- ルート自身がプロジェクトの条件を満たす場合は、ルートを 1 件のプロジェクトとして扱う。
- `excludedPaths` にあるパスとその配下は一覧に出さない。
- 手動追加（`add_project`）は条件を問わず登録できる。存在するディレクトリであること、登録済みでないことを確認する（`invalid_path` / `already_registered`）。
- `Project.id` は正規化した絶対パスの SHA-256 の先頭 16 桁（16 進）。`name` はフォルダ名。
- 登録済みのフォルダが消えた場合は一覧から黙って消さず、`state = "missing"` で残す（スキャンで見つかったものは次回のスキャンで外れてよい。手動追加したものと、`ProjectSettings` を持つものは残す）。
- `scan_projects` は有効な全ルートを再探索し、状態も取得して `Workspace` を返す。`add_scan_root` などの変更系コマンドも同じ結果を返す。

## 4. 状態の判定

`ProjectStatus.state` は `types.ts` の `SyncState` に書いた優先順位で 1 つ選ぶ。数値（`ahead`、`behind`、`changes`）は主状態に関係なく常に実際の値を入れる。

- `refresh_projects` はローカルの情報だけで状態を作る（ネットワークに出ない）。`projectIds = null` は全件。プロジェクト単位で最大 8 並列に処理する（直列にしない）。結果を返し、変化のあったプロジェクトは `revia:project-updated` も送る。
- 状態の取得は軽く保つ。1 プロジェクトあたりの Git 起動回数を抑え（目安 10 回以内）、ファイルごとの `numstat` や未追跡ファイルの読み込みは行わない。未追跡はディレクトリ単位（`--untracked-files=normal`）で数える。行数などの詳細は `get_changes` でだけ求める。
- `remoteError` は Revia が行った直近のネットワーク操作（Fetch / Pull / Push）の結果から決め、成功したら `null` に戻す。メモリ上に保持し、再起動で消えてよい。
- `fetchedAt` は Revia の Fetch が成功した時刻。記録がなければ `.git/FETCH_HEAD` の更新時刻、それもなければ `null`。
- `github` は URL が `github.com` を指すリモートから求める（`https://github.com/o/r(.git)`、`git@github.com:o/r(.git)`、`ssh://git@github.com/o/r(.git)`）。

### 操作先の解決

現在のブランチの Push 先と Pull 元を、次の順で決めて `targets` に入れる。

1. **saved** — `ProjectSettings.branchTargets[現在のブランチ]` の設定。リモートが Git に存在しなければ `savedInvalid = "remote_missing"`。Pull 元でリモート追跡ブランチが存在しなければ `"branch_missing"`。無効な場合は `target = null` とし、下の候補へ自動で切り替えない（UI が再選択を求める）。Push 先のブランチがリモートにまだない場合は有効（`exists = false`）。
2. **upstream** — Git の追跡設定。Push 先は `@{push}`、なければ `@{upstream}`。
3. **remote_default** — 既定リモートに同名のブランチがある。既定リモートは `defaultRemote` 設定 → リモートが 1 つならそれ → `origin`。
4. **inferred** — （Push 先のみ）既定リモートに同名のブランチを新しく作る（`exists = false`）。
5. 既定リモートを決められない（リモートが複数あり `origin` がない）場合は `target = null`。

`ahead` は Push 先、`behind` は Pull 元との差。比較対象のリモート追跡ブランチがなければ `null`。

`divergence` は、Pull 元に新しいコミットがあり（`behind > 0`）、手元にも新しいコミットか未コミットの変更がある場合だけ入れ、それ以外は `null`。`localAt` は、手元の新しいコミット（`ahead > 0` のとき HEAD）のコミット日時と、変更のあるファイル（先頭 500 件）の更新日時のうち最も新しいもの。`remoteAt` は Pull 元の先頭のコミット日時。どちらも求められなければ `null`。UI はこの 2 つを比べ、1 分以上新しい側を「おすすめ」として示す（日時は各コンピューターの時計による）。

Revia の保存設定と Git の upstream は別物として扱う。`set_branch_target` は Revia の設定だけを変え、Git の設定には触れない。Git の追跡設定を変えるのは `set_upstream` と、`push` の `setUpstream = true` だけ。

## 5. Git の実行規約

- `git` と `gh` は引数配列で起動する。シェルを介さない。実行ファイルは `PATH` に加えて、OS ごとの既定の場所から探す（GUI から起動したアプリはシェルの `PATH` を引き継がないことがある）。macOS: `/opt/homebrew/bin`、`/usr/local/bin`、`/usr/bin`。Linux: `/usr/bin`、`/usr/local/bin`、`/snap/bin`、`/home/linuxbrew/.linuxbrew/bin`、`~/.local/bin`。Windows: `PATHEXT` の拡張子を補い、`%ProgramFiles%\Git\cmd`、`%ProgramFiles%\GitHub CLI`、`%LOCALAPPDATA%\Programs\Git\cmd`、scoop と winget の shim も見る。
- Windows・macOS・Linux で同じ動作にする。Windows では子プロセスをコンソールウィンドウなしで起動する（`CREATE_NO_WINDOW`）。正規化したパスから `\\?\` の接頭辞を取り除き、Git に渡すパスと表示するパスを通常の形にそろえる。OS 固有の処理は `cfg` で分け、どの OS でもコンパイルできる状態を保つ。
- 環境変数: `GIT_TERMINAL_PROMPT=0`、`GIT_OPTIONAL_LOCKS=0`（読み取り系）、`LC_ALL=C`、`GCM_INTERACTIVE=never`。SSH は `GIT_SSH_COMMAND` に `-o BatchMode=yes` を加え、入力待ちで止まらないようにする。出力は `-c core.quotepath=false` と `-z` で扱い、日本語のパスを壊さない。
- GitHub の HTTPS リモートへの認証は、`gh` が使える場合に限り、実行ごとに `-c credential.https://github.com.helper= -c credential.https://github.com.helper=!<gh の絶対パス> auth git-credential` を渡して gh に任せる（github.com だけが対象。他のホストの設定には触れない）。`gh` が見つからない場合は何も足さず、利用者の Git 設定に従う（エラーにしない）。グローバルの Git 設定は書き換えない。
- パス引数は `--` の後ろに置き、`GIT_LITERAL_PATHSPECS=1` を付ける。プロジェクト外を指すパス（絶対パス、`..`）は `invalid_path`。
- ブランチ名・リモート名は `-` で始まるもの、制御文字を含むもの、`git check-ref-format` を通らないものを `invalid_ref_name` で拒否する。
- リモート URL は `https://`、`http://`、`ssh://`、`git://`、`file://`、SCP 形式（`[user@]host:path`）、絶対パスを受け付ける。`ext::` や `fd::` など `::` を含む転送指定、`-` で始まる値、制御文字を含む値は `invalid_input` で拒否し、`detail` に理由を入れる。`-c protocol.ext.allow=never` を付ける。
- 読み取りのローカル操作は 60 秒、ネットワーク操作は 180 秒でタイムアウト（`timeout`）。Clone は時間で打ち切らず、中止だけ受け付ける。
- 作業ツリーやインデックスを書き換えるローカル操作（`checkout` / `switch`、`merge`、`rebase`、`cherry-pick`、`revert`、`reset`、`commit`、`add`、`stash`、それらの `--continue` と `--abort`）は、途中で強制終了すると書きかけの状態やロックファイルが残るので、10 分まで待つ。10 分を超えたら終了を要求し（Unix は SIGTERM。Windows は kill）、5 秒待っても終わらなければ kill して `timeout`。
- すべての `git` に `-c core.fsmonitor=false` を付ける。リポジトリの中の設定（`.git/config`）だけで外部プログラムが起動する経路を、状態の読み取りでは通さないため（自動検出したフォルダにも `git status` を実行する）。
- リモート URL に認証情報を含めることは受け付けない（`invalid_input`）。`https://` / `http://` / `git://` はユーザー情報（`user@`、`user:password@`、`token@`）があれば拒否。`ssh://` と SCP 形式は、ユーザー名だけ（`git@`）は可、パスワード付き（`user:password@`）は拒否。URL の認証情報はプロセスの引数と `.git/config` に平文で残るため。
- ほかの Git プロセスがリポジトリを使用中で失敗した場合（Git の出力に `index.lock` などの `.lock` と `File exists`、または `Another git process seems to be running`）は `repo_locked`。Revia はロックファイルを消さない。
- 競合（`conflict`）は Git の出力の `CONFLICT (` で判定する。`would be overwritten` は、パスに conflict という語が含まれていても `dirty_worktree`。
- Windows で `.cmd` / `.bat` を起動するときは、`cmd.exe /C` を自分で組み立てない（引数の `&` や `|` が cmd に解釈される）。Rust 標準ライブラリのバッチファイル向けのエスケープに任せ、安全に渡せない引数はエラー（`invalid_input`）にする。
- 失敗の分類は終了コードと標準エラーから行う: 認証（`auth_failed`）、権限（`permission_denied`。HTTP 403 を含む）、到達不能（`network_unreachable`。名前解決の失敗、接続拒否、接続タイムアウト、`Network is unreachable`、`Failed to connect` を含む）、リポジトリなし（`remote_not_found`）、non-fast-forward の拒否（`push_rejected`）、空き容量（`disk_full`）など。分類できなければ `unknown` に `detail` を付ける。
- パス引数のうち Git に渡すだけのもの（コミット対象、差分対象など）は、最後の要素がシンボリックリンクでもよい（Git はリンクそのものを扱う）。途中の要素がシンボリックリンクの場合と、Revia 自身がファイルを読む・開く場合は拒否する。
- 大きな入力で固まらないよう上限を設ける: `get_changes` は 5,000 件まで、未追跡ファイルの行数計算と競合マーカーの検査は 1MB まで（超える場合は `additions = null` / `hasMarkers = true`）、未追跡ファイルの差分は 512KB まで読んで `truncated = true`。
- 操作が失敗したら、成功したように見せない。必ず状態を取り直して `revia:project-updated` を送る。

## 6. コマンドの意味

### 読み取り

| コマンド | 内容 |
| --- | --- |
| `get_app_state` | 起動時に 1 回呼ぶ。保存済みの一覧をすぐ返す（`status` は前回の値がなければ `null`）。状態の取得は `refresh_projects` で行う |
| `get_environment` | `git` / `gh` の有無とバージョンを調べ直す |
| `get_project_detail` | 状態を取り直し、ローカルとリモートのブランチ、stash を返す |
| `get_changes` | 変更のあるファイル。`additions` / `deletions` は HEAD との差 |
| `get_diff` | `worktree` は HEAD と作業ツリーの差分（未追跡ファイルは全行追加、コミットがなければ空ツリーとの差分）。5,000 行または 512KB で打ち切り `truncated = true` |
| `get_commit_log` | `rev = null` は HEAD。`limit` は 1〜200 |
| `get_commit_detail` | コミットと変更ファイル。ファイルの差分は `get_diff` の `commit` で取る |
| `get_conflicts` | 途中の操作と未解決ファイル。`incoming` は取り込み中のブランチ名（`MERGE_MSG` や `git name-rev` から求め、分からなければ短縮 SHA） |
| `preview_merge` / `preview_rebase` / `preview_reset` | 実行せずに影響を調べる。作業ツリーとインデックスを変更しない |

`preview_merge` の競合検査は `git merge-tree --write-tree` を使い、使えない Git では `status = "unknown"` とする。

`fingerprint` は 2 系統ある。`preview_rebase` / `preview_reset` の値は、対応する `rebase` / `reset` コマンドにそのまま渡す。操作エンジン（`start_operation`）が照合するのは `plan_operation` が返した値で、`preview_merge` の値とは互換性がない。Merge を実行するときは、`preview_merge` で内容を見せたあと `plan_operation` を呼び、その `fingerprint` を付ける。

### 変更（単体）

すべて対象リポジトリのロックを取り、操作後に状態を取り直して `revia:project-updated` を送り、履歴に記録する。`ProjectDetail` を返すものは最新の内容を返す。

| コマンド | 内容と安全上の決まり |
| --- | --- |
| `init_repository` | `git init`。`initialBranch = null` は `main`。既にリポジトリなら何もしない |
| `create_branch` | `startPoint = null` は HEAD。同名があれば `branch_exists`。`checkout = true` で切り替える（未コミットの変更は持ち越す。持ち越せなければ `dirty_worktree`） |
| `checkout_branch` | `branch` はローカル名、または `origin/x` 形式（同名のローカルブランチを追跡付きで作って切り替える。既にあればそれへ切り替える）。未コミットの変更がある場合: `abort` は `dirty_worktree` で拒否、`carry` は変更を持ったまま切り替え（Git が拒否したら `dirty_worktree`、何も変えない）、`stash` は未追跡を含めて退避してから切り替える |
| `rename_branch` | `git branch -m`。移行先が存在すれば `branch_exists`。保存済みの `branchTargets` のキーも付け替える |
| `delete_branch` | 現在のブランチは不可（`invalid_input`）。`force = false` は `-d`。未マージなら `branch_not_merged`（`detail` に未マージのコミット数）。`force = true` は `-D` |
| `delete_remote_branch` | `git push <remote> --delete <branch>`。ネットワーク操作 |
| `set_upstream` | Git の追跡設定を変更。`null` で解除 |
| `add_remote` | 同名があれば `remote_exists`。既存のリモートを上書きしない |
| `set_remote_url` / `rename_remote` / `remove_remote` | 既存リモートの変更。`rename_remote` と `remove_remote` は保存済みの `branchTargets` と `defaultRemote` を追随させる（削除時は該当設定を残し、`savedInvalid` で検知させる） |
| `mark_resolved` | 指定したパスを `git add` する |
| `continue_in_progress` | 未解決ファイルがあれば `merge_conflict`。Merge は `git commit --no-edit`、Rebase / Cherry-pick / Revert は `--continue` |
| `abort_in_progress` | `git merge --abort` など。途中の操作がなければ `invalid_input` |
| `rebase` | 作業ツリーが汚れていれば `dirty_worktree`。`fingerprint` が合わなければ `state_changed`。競合したら停止して `merge_conflict` を返す（中断はしない） |
| `cherry_pick` | 同上。競合で停止したら `merge_conflict` |
| `reset` | `hard` は `safety.allowHardReset = false` のとき `feature_disabled`。`fingerprint` 必須 |
| `discard_changes` | `paths = null` は全変更。追跡ファイルを HEAD に戻す。`includeUntracked = true` のときだけ未追跡ファイルを削除する。無視ファイルは消さない。途中の操作があれば `operation_in_progress` |
| `stash_save` / `stash_apply` / `stash_drop` | stash は SHA で指定し、実行時に一覧と照合する。`stash_apply` が競合したら `merge_conflict`（`drop = true` でも stash は消さない） |

フロントエンドは確認画面を出してからこれらを呼ぶ。Rust 側も上記の拒否条件を必ず守る（UI の確認だけに頼らない）。

## 7. 操作エンジン

Fetch / Pull / Push / Commit / 同期 / Merge / 公開 / Clone は、単体でも一括でも同じ経路で実行する。

- `plan_operation(request)` — 実行せずに、項目ごとの予定（`PlanItem`）を返す。ネットワークに出ない。作業ツリーを変更しない。
- `start_operation(request)` — すぐに `OperationSnapshot`（全項目 `queued`）を返し、バックグラウンドで実行する。項目が変わるたびに `revia:operation` の `item` を、開始・中止受付・完了時に `state` を送る。
- `cancel_operation(operationId)` — 中止を受け付ける。`queued` の項目は `cancelled` にする。実行中の項目は、ネットワーク手順（fetch / push / clone / pull の取得部分）なら子プロセスを止めて `cancelled`、ローカルの書き込み手順（commit / merge / checkout）は完了まで待ってから実際の結果を入れる。完了済みの項目は変えない。`completed` には実際に済んだ手順だけを入れる。
- `get_operation` — 実行中または直近 20 件の完了済み操作のスナップショット。なければ `null`。

### 実行の決まり

- 項目同士は独立。ある項目の失敗・競合は他の項目に影響しない。
- 同時実行数は `general.concurrency`。同じプロジェクトの項目は直列。
- `fingerprint` が指定されていれば、実行直前に同じ方法で計算し直して照合する。フロントエンドは `risk` が `safe` でない項目にだけ付ける（安全な操作は実行時に状況を判定し直すので、作業中のファイル変更で止めない）。合わなければ何も実行せず `needs_confirmation`（`reason = "state_changed"`）。`fingerprint` はローカルの状態（HEAD、現在のブランチ、対象の参照の SHA、変更の有無、解決した操作先）から作る不透明な文字列。
- `merge` と、`pull` の `merge` / `rebase`、`push` の `force = true` は `fingerprint` 必須（なければ `invalid_input` で `start_operation` を拒否）。
- 進捗 `progress` は Git の `--progress` 出力から得た実測値だけを入れる（fetch / clone は `Receiving objects`、push は `Writing objects` の割合）。得られなければ `null`。イベントは 150ms 以上の間隔に間引く。
- 各項目の完了時に、状態を取り直して `revia:project-updated` を送り、履歴に 1 行記録する。

### 項目の最終状態

| 状態 | 意味 |
| --- | --- |
| `succeeded` | 予定した手順が完了した（同期で「既に最新」の場合も含む。`completed` で内容が分かる） |
| `skipped` | 対象がない（`reason = "nothing_to_do"`）。Push するコミットがない、コミットする変更がない等 |
| `needs_confirmation` | 安全に自動実行できないため何もしなかった、または安全な手順だけ済ませて止めた。`reason` に理由 |
| `conflict` | Merge / Pull が競合で停止した。作業ツリーは競合状態のまま（自動で解決も中断もしない） |
| `failed` | エラー。`error` に内容 |
| `cancelled` | 中止 |

### 種類ごとの動作

**fetch** — すべてのリモートを取得する（`--prune` はしない）。リモートがなければ `needs_confirmation`（`no_remote`）。一部のリモートが失敗しても残りは取得し、1 つでも失敗があれば最初のエラーで `failed` にする（`completed` に fetch は入れない）。

**pull** — Pull 元は解決済みの `targets.pull`。`prefer`（`"local"` / `"remote"`、省略は `null`）は `mode = "merge"` のときだけ指定でき、`-X ours` / `-X theirs` を付けて Merge する（重なった箇所は手元 / リモートの内容を採る）。ほかの `mode` に付けたら `invalid_input`。プレビューの Pull の手順に `prefer` を入れ、競合の見込みは `conflicts_resolved` で示す。未解決なら `needs_confirmation`（`target_unresolved` / `target_invalid`）。取得してから: 既に最新なら `skipped`。`ff_only` は fast-forward できなければ `needs_confirmation`（`diverged`）。`merge` は `git merge`、競合したら `conflict`。`rebase` は `git rebase`、競合したら `conflict`。`merge` / `rebase` は未コミットの変更があれば実行せず `needs_confirmation`（`dirty_worktree`）。Detached HEAD・途中の操作あり・未解決の競合ありも `needs_confirmation`。

**push** — Push 先は `targets.push`。通常は `git push <remote> <local>:<branch>`。`setUpstream = true` で `-u`。リモート側が進んでいて拒否されたら `failed`（`push_rejected`）。`force = true` は `--force-with-lease=<branch>:<プレビュー時のリモート SHA>` のみ使い、`safety.allowForcePush = false` なら `needs_confirmation`（`feature_disabled`）。`--force` は使わない。Push するコミットがなければ `skipped`。

**commit** — `paths = null` はすべての変更（未追跡を含む）。`paths` 指定時は、そのパスの現在の作業ツリーの内容だけをコミットし、他の変更はそのまま残す。変更がなければ `skipped`。`message` が空なら `invalid_input`。`user.name` / `user.email` が未設定なら `failed`（`identity_missing`）。未解決の競合があれば `needs_confirmation`（`conflicted`）。

**sync** — `general.sync` の設定に従って、コミットから Pull・Push までをまとめて行う。

1. 事前確認。`missing` / `not_initialized` / `no_remote` / `detached` / `in_progress` / `conflicted` → 何もせず `needs_confirmation`。Push 先か Pull 元の保存済み設定が無効 → `target_invalid`。Push 先を決められない → `target_unresolved`。Pull 元がないだけ（ブランチがまだリモートにない）の場合は止めず、Pull の手順を省く。
2. Fetch。対象は Pull 元と Push 先のリモートだけ（無関係なリモートの障害で止めない）。
3. Pull 元より遅れていて fast-forward できる → `merge --ff-only`。Git が未コミットの変更との衝突で拒否したら、`commit = true` なら 4 のあと 5 で取り込み、`false` なら `needs_confirmation`（`dirty_worktree`）。
4. `commit = true` で変更があれば、すべての変更（未追跡を含む。`.gitignore` に従う）をコミットする。ただし、まだ追跡していないファイルのうち秘密情報らしいもの（publish の `sensitive_files` と同じ規則）はコミットせずに残し、プレビューで `skips_sensitive_files` を示す。メッセージは `commitMessage`。空なら変更したファイルから作る（`Update a.txt`、`Update a.txt and b.txt`、`Update a.txt, b.txt and 3 more files`）。`user.name` / `user.email` がなければ `failed`（`identity_missing`）。`commit = false` なら未コミットの変更には触れない。
5. 両方に変更がある（分岐） → `diverged` に従う。`stop` は止めて `needs_confirmation`（`diverged`）。`merge` は `git merge --no-edit` で取り込み、競合したら `conflict` で止める（作業ツリーは競合状態のまま）。`prefer_local` / `prefer_remote` は `-X ours` / `-X theirs` を付けて取り込み、同じ箇所を両方が変えていたら手元 / リモートの内容を採る（ファイルの削除と変更がぶつかった場合など、Git が自動で決められない競合は `conflict` で止める）。追跡しているファイルに未コミットの変更が残っていれば、取り込まずに `needs_confirmation`（`dirty_worktree`）。Pull の手順は `mode = "merge"`、`prefer` に `"local"` / `"remote"` / `null`。
6. Push 先より先行している → 通常の Push（force なし）。Push 先のブランチがリモートにない場合は新規作成する（Git の追跡設定は変えない）。
7. 履歴を書き換える操作（Rebase、強制 Push、Reset）はしない。退避（stash）もしない。

プレビューの手順は実行する順に並べる（分岐していなければ `fetch` → `pull` → `commit` → `push`、分岐していれば `fetch` → `commit` → `pull` → `push`）。分岐していて `diverged` が `stop` 以外なら `risk` は `caution`。取り込みで競合が見込まれる場合、`merge` は `conflicts_likely`、`prefer_local` / `prefer_remote` は `conflicts_resolved`（`prefer` とパス）を付ける。

**merge** — `target` が現在のブランチでなければ先に切り替える（未コミットの変更があれば `needs_confirmation`（`dirty_worktree`））。`default` は fast-forward できればそうし、できなければ Merge コミットを作る。`no_ff` は常に Merge コミット。`ff_only` はできなければ `needs_confirmation`（`diverged`）。競合したら `conflict` で停止。既に取り込み済みなら `skipped`。

**publish** — GitHub にリポジトリを作って結び付ける。`gh` 未ログインなら `needs_confirmation`（`not_authenticated`）。手順: Git 未初期化なら `init` → 最初のコミットを作る場合で `ignore` があれば `.gitignore` に追加する（`ignore`。§12） → コミットがなく `initialCommitMessage` があれば全ファイルを最初のコミットにする → `gh repo create`（同名があれば `failed`（`repo_exists`）。既存のリポジトリを使い回さない） → `origin` を追加（`origin` が既にあれば、リポジトリを作る前に `needs_confirmation`（`remote_exists`）。上書きしない） → コミットがあれば現在のブランチを Push して追跡先に設定。プレビューでは `sensitive_files`（`.env`、`.env.*`（`.env.example` を除く）、`*.pem`、`*.key`、`*.p12`、`id_rsa*`、`id_ed25519*`、`.npmrc`、`.netrc`、`credentials.json` など、最初のコミットに入るもの）、`no_gitignore`、`large_files`（50MB 超）、`many_files`（5,000 超）、`public_repo` を警告する。警告があるか公開リポジトリなら `risk = "caution"`、`sensitive_files` があれば `"danger"`。

**clone** — `parentDir/folderName` が既にあれば `destination_exists`。完了したら、スキャンルートの配下ならスキャン由来、そうでなければ手動追加として登録し、`revia:projects-changed` を送る。中止した場合は作りかけのフォルダを削除する（Revia が作成したものに限る）。

### プレビューの `risk`

- `safe`: fetch、fast-forward の pull、通常の push、commit、clone、分岐を取り込まない同期。
- `caution`: merge、`merge` モードの pull、分岐を取り込む同期、publish、新しいリモートブランチを作る push 以外で警告がある場合。
- `danger`: force push、`rebase` モードの pull で Push 済みコミットを書き換える場合、`sensitive_files` のある publish。

### 状態が変わっていた場合の安全策

- **pull と sync**: Fetch のあと、Merge / Rebase / 早送りの直前に、ブランチ・HEAD・作業ツリーの状態が実行開始時に確かめた内容から変わっていないことを確かめ直す。変わっていたら何もせず、`fingerprint` が合わなかった場合と同じ結果にする。競合の見込み（プレビューの警告）は Fetch 後の内容では求め直せないので、実行結果で伝える。同期が途中でコミットした場合は、そのコミットの直後の状態を基準にして確かめる。
- **push**: `push_rejected` で失敗したら、そのリモートを Fetch して状態を取り直す（一覧の表示が「Pull 待ち」や「分岐」に変わる）。この Fetch の失敗は結果に含めない。
- **変更を退避して checkout**: `stash push` の前後で `refs/stash` を比べ、この操作が作った stash だけを復元と削除の対象にする。何も退避されなかった場合（`refs/stash` が変わらない）は、既存の stash に触れない。

## 8. GitHub

認証情報の保管は GitHub CLI（`gh`）に任せる。トークンは gh が OS の資格情報ストア（macOS はキーチェーン）に保存し、Revia は保存しない。`gh auth token` を呼ばない。

ログインと権限の追加は、Revia の OAuth App のデバイスフローで行う（GitHub の認可画面に「Revia」と表示される）。Client ID は `Ov23lid8N3XXfT9C0QTa`（公開情報）。Client secret は使わない。受け取ったトークンは `gh auth login --with-token` の標準入力に 1 回渡すだけで、Revia はメモリ以外に置かない。

| コマンド | 内容 |
| --- | --- |
| `github_status` | `gh` の有無、ログイン状態、アカウント。`refresh = false` は 5 分以内のキャッシュを返してよい。`gh` がなければ `cliInstalled = false`（エラーにしない）。`tokenStorage` は gh がトークンを置いている場所で、`gh auth status` の「Logged in to github.com account … (…)」（有効なアカウントの行）の括弧の中から決める: `keyring` → `"keyring"`、`GH_TOKEN` / `GITHUB_TOKEN` → `"environment"`、それ以外（設定ファイルのパス）→ `"file"`、未ログインか読み取れない → `null`。`"file"` は平文の設定ファイルなので、UI が注意を出す |
| `install_github_cli` | `gh` が見つからないときに、GitHub CLI を Revia のデータフォルダ（`tools/gh/bin/`）へ入れて `GithubStatus` を返す。既に `gh` があれば何もせず状態を返す。手順: `GET https://api.github.com/repos/cli/cli/releases/latest`（認証なし）→ OS と CPU に合う公式のアーカイブ（macOS と Windows は `.zip`、Linux は `.tar.gz`）を選ぶ → 更新と同じ決まり（許可したホスト、リダイレクトは 5 回まで）で取得し、API が示す大きさと SHA-256 で照合する（SHA-256 のないものは入れない）→ OS の `tar`（Windows は `System32\tar.exe`）で展開 → `gh --version` が起動することを確かめてから置く。途中で失敗したら何も残さない。起動時に `gh` が PATH などになければ、この場所の `gh` を使う。同時に 2 つ目を呼ぶと `busy`。合うアーカイブがなければ `unknown`、照合に失敗したら `verification_failed`、API の制限は `rate_limited` |
| `github_login_start` | 引数 `scopes: string[] \| null`。下の「ログインの流れ」のとおり、デバイスフローでログインする（`null`）か、権限を足す（指定できるのは `delete_repo` だけ。それ以外や空の配列は `invalid_input`。未ログインなら `not_authenticated`）。同時に 1 件まで（`busy`）。`gh` がなければ `gh_not_installed`。戻り値は `{ loginId }` で、進行は `revia:github-login` で送る |
| `github_login_cancel` | 進行中のログインを止めて `cancelled` を送る（デバイスフローの待機も、gh の子プロセスも） |
| `github_logout` | `gh auth logout --hostname github.com`。gh 自体のログイン状態が消えることを UI が説明する |
| `github_list_repos` | ログイン中のユーザーがアクセスできるリポジトリ（所有・コラボレーター・Organization）。更新日の新しい順。`refresh = false` は 5 分以内のキャッシュ。`localProjectIds` はリモート URL が一致する登録済みプロジェクト |
| `github_list_owners` | リポジトリを作成できる先（本人と所属 Organization） |

未ログインは `not_authenticated`、レート制限は `rate_limited`、`gh` がなければ `gh_not_installed`。

### ログインの流れ

1. `POST https://github.com/login/device/code` に `client_id` と `scope` を送る（`application/x-www-form-urlencoded`、`Accept: application/json`）。`scope` は通常のログインで `repo read:org workflow`、権限の追加では `repo read:org workflow delete_repo`。
2. 応答の `user_code` を `revia:github-login`（`code`）で送る。`verificationUrl` は固定の `https://github.com/login/device`（応答の `verification_uri` がこれと違えば `failed`（`auth_failed`））。ブラウザは Revia 側で開く（`open_url` と同じ仕組み）。
3. `interval` 秒ごと（5 秒未満なら 5 秒）に `POST https://github.com/login/oauth/access_token` へ `client_id`、`device_code`、`grant_type=urn:ietf:params:oauth:grant-type:device_code` を送る。`authorization_pending` は続行、`slow_down` は間隔を 5 秒延ばして続行、`access_denied` は `cancelled`、`expired_token` は `failed`（`timeout`）、それ以外のエラーは `failed`（`auth_failed`。`detail` は GitHub が返した `error` の値だけ）。通信できないときは `failed`（`network_unreachable`）。10 分（応答の `expires_in` が短ければそちら）で `failed`（`timeout`）。
4. トークンを受け取ったら、`gh auth login --hostname github.com --git-protocol https --with-token` の標準入力に渡す（引数・環境変数・ファイルには置かない）。成功したら `github_status(true)` を取り直して `succeeded`。gh が失敗したら `failed`（gh の出力から分類）。

決まり:

- トークンと `device_code` を、イベント・ログ・履歴・エラーの `message` と `detail` に含めない。`user_code` を出すのはイベントの `userCode` だけ。トークンを gh に渡したあとは、持っていたバッファを上書きしてから捨てる。
- 通信先は上の 2 つの URL だけ。HTTPS のみ。リダイレクトをたどらない。応答は 64KB まで。1 回の要求は 15 秒でタイムアウト。
- Revia の OAuth App が使えないとき（デバイスコードの要求に、`device_flow_disabled`・`incorrect_client_credentials`・`unauthorized_client` のいずれか、または HTTP 401・403・404・422 が返った）は、同じログインの中で gh 自身のログインに切り替える（`gh auth login --hostname github.com --git-protocol https --web --skip-ssh-key`。権限の追加は `gh auth refresh --hostname github.com --scopes delete_repo`。出力からワンタイムコードを読み取って `code` を送る。この場合、認可画面の表示は「GitHub CLI」になる）。OAuth の設定を持たない構成（テスト）でも、この gh のログインを使う。

## 9. その他

- `pick_folder` — OS のフォルダ選択ダイアログ。キャンセルは `null`。
- `pick_background_image` — 引数 `filterLabel: string | null`（ダイアログに出すファイル種別の表示名。UI が表示言語に合わせて渡す。`null` や空文字なら `Images`）。OS のファイル選択ダイアログ（png / jpg / jpeg / webp / gif / avif）。選ばれたファイルが通常のファイルで 30MB 以下であることを確かめ、`app_data_dir()/backgrounds/<uuid>.<拡張子>` へ複製して `BackgroundImage` を返す。複製に成功したら、同じフォルダの他のファイルは削除する（保持するのは常に 1 枚）。キャンセルは `null`。大きすぎる・種類が違う場合は `invalid_input`。元のファイルは変更しない。
- `read_background_image` — `id` が `<uuid>.<拡張子>` の形であることを確かめ、`backgrounds` フォルダ内のそのファイルの内容をバイト列（`tauri::ipc::Response`）で返す。なければ `path_not_found`。フォルダの外は読まない。
- `clear_background_image` — `backgrounds` フォルダ内のファイルをすべて削除する。
- `open_project` — OS ごとの標準の方法を使う。`file_manager` は Finder / エクスプローラー / 既定のファイルマネージャーで表示、`terminal` は既定のターミナルで開く（Windows は Windows Terminal、なければ `cmd`。Linux は `x-terminal-emulator`、`gnome-terminal`、`konsole` などを順に探す）、`editor` は `general.editor`（`null` なら検出した最初のエディタ、なければ OS 既定）で開く、`github` はリポジトリのページを開く（`github` がなければ `invalid_input`）。`relativePath` はプロジェクト内のファイルを指す（コンフリクトしたファイルをエディタで開く用途）。
- `open_url` — `https://github.com/`、`https://x.com/`、`https://creativecommons.org/` のいずれかで始まる URL だけ開く。それ以外と、制御文字・`\`・`@` を含むものは `invalid_input`。
- `list_editors` — 検出したエディタ（VS Code、Cursor、Zed、Sublime Text、Xcode など）。
- `get_history` — 新しい順。`before` は前回最後のエントリの `id`。`projectId` で絞り込める。
- `clear_history` — 履歴を全件削除する。

## 10. 更新

Revia の公開版は GitHub のリリースで配布する。確認先のリポジトリは `Shake1227/Revia` に固定し、フロントエンドからは受け取らない。Git / gh を介さずに Revia 自身がネットワークへ出るのはこの節のコマンドだけで、送るのは `User-Agent: Revia/<バージョン>` を付けた取得リクエストだけ（認証情報、利用者やプロジェクトの情報は送らない）。HTTP は OS 標準の TLS とシステムのプロキシ設定を使う。

| コマンド | 内容 |
| --- | --- |
| `check_update` | 公開済みの最新リリースを調べて `UpdateInfo` を返し、`download_update` のためにメモリへ保持する |
| `download_update` | 直近の `check_update` が選んだインストーラーを取得し、検証して `UpdateDownload` を返す |
| `cancel_update_download` | 取得を中止する。取得中でなければ何もしない |
| `install_update` | 直近の `download_update` で取得したファイルを開く |

### `check_update`

- `GET https://api.github.com/repos/Shake1227/Revia/releases/latest`。ヘッダーは `Accept: application/vnd.github+json` と `X-GitHub-Api-Version: 2022-11-28`。認証なし。15 秒でタイムアウト（`timeout`）。
- HTTP 404 は `latest = null`（リリースがまだない、またはリポジトリが非公開）。`draft` か `prerelease` が true のもの、タグ名を SemVer として読めないものも `latest = null`。
- HTTP 403 / 429 は `rate_limited`。接続できなければ `network_unreachable`。それ以外の状態コードや読めない JSON は `unknown`（`detail` に `GitHub API returned HTTP <状態コード>` など）。
- `version` はタグ名の先頭の `v` を除いたもの。`available` は SemVer の順序で `latest.version > currentVersion` のときだけ true。`currentVersion` はアプリのバージョン（`Environment.appVersion` と同じ値）。
- `name` と `notes` は空なら `null`。`notes` は本文をそのまま返し、20,000 文字を超える分は切り捨てる。
- `url` は API の `html_url`。`https://github.com/Shake1227/Revia/releases/` で始まらなければ `https://github.com/Shake1227/Revia/releases/tag/<タグ名>` を使う。
- `asset` は次の規則で 1 つ選ぶ（大文字と小文字を区別しない）。候補にするのは、ファイル名が英数字と `.`、`_`、`-`、`+` だけでできていて 120 文字以内、`.` で始まらず、`browser_download_url` が `https://github.com/Shake1227/Revia/releases/download/` で始まるものだけ。
  - macOS: `.dmg`。名前に `universal` を含むものを優先し、なければ CPU に合うもの（`aarch64` / `arm64`、`x64` / `x86_64`）。
  - Windows: `-setup.exe`（NSIS）のうち CPU に合うもの（`x64`、`arm64`）。`.msi` は選ばない。
  - Linux: 環境変数 `APPIMAGE` があれば `.AppImage`。なければ `dpkg` が見つかれば `.deb`、`rpm` が見つかれば `.rpm`、どちらもなければ `.AppImage`。CPU に合うもの（`amd64` / `x86_64`、`arm64` / `aarch64`）。
  - 合うものがなければ `null`（UI はリリースページを開く案内に切り替える）。
- `hasDigest` は、API の `digest` が `sha256:` に 64 桁の 16 進数が続く形のときだけ true。

### `download_update`

- 直近に成功した `check_update` の `asset` を取得する。なければ `invalid_input`。同時に 1 件まで（`busy`）。URL やファイル名は引数で受け取らない。
- 保存先は `app_cache_dir()/updates/<ファイル名>`。`<ファイル名>.part` へ書き、検証が済んでから `rename` する。
- リダイレクトは自動でたどらず、5 回まで自分でたどる。そのたびに `https` であること、ホストが `github.com`、`objects.githubusercontent.com`、`release-assets.githubusercontent.com` のいずれかであることを確かめ、外れたら `invalid_input`。
- 受信は 1GB で打ち切る。受信したバイト数が API の `size` と違えば `verification_failed`。`digest` があれば SHA-256 を照合し、違えば `verification_failed`。失敗したときは `.part` を削除する。
- 同じ名前のファイルが既にあり、`digest` があって照合が通る場合は取得し直さない（進捗は完了分を 1 回送る）。
- 進捗は `revia:update-progress`（`received` と `total`）。150ms 以上の間隔に間引き、完了時に必ず 1 回送る。
- `cancel_update_download` で中止されたら `.part` を削除して `cancelled`。
- 成功したら `updates` フォルダの他のファイルを削除する（保持するのは常に 1 つ）。`verified` は SHA-256 を照合できたときだけ true。
- 失敗の分類: 接続できない `network_unreachable`、応答が止まった `timeout`（60 秒間 1 バイトも届かない）、空き容量 `disk_full`、その他の書き込み `io_error`、HTTP の異常は `unknown`。

### `install_update`

- 直近に成功した `download_update` のファイルを使う。なければ `invalid_input`、ファイルが消えていれば `path_not_found`。パスは引数で受け取らない。
- macOS（`dmg`）は既定の方法で開く（ディスクイメージがマウントされる）。Windows（`nsis`）はインストーラーを起動して待たずに戻る。Linux はファイルマネージャーでそのファイルを表示する。
- Revia 自身は終了しない。次の手順は UI が案内する。

## 11. 連携の解除と削除

利用者のファイルと GitHub のリポジトリを消す操作をまとめる。Revia はファイルを完全には削除せず、必ず OS のゴミ箱（macOS のゴミ箱、Windows のごみ箱、Linux の freedesktop Trash）へ移す。ゴミ箱へ移せなかった場合に、完全な削除へ切り替えない（`io_error` を返す）。GitHub のリポジトリの削除だけは取り消せない。

| コマンド | 内容 |
| --- | --- |
| `preview_removal` | 消えるものと残るものを調べて `RemovalPreview` を返す。ローカルの情報だけで作り、ネットワークに出ない。何も変更しない |
| `check_repository_deletion` | `RemovalPreview.github` のリポジトリを GitHub 側で削除できるかを調べる（ネットワークに出る） |
| `remove_git` | `.git` をゴミ箱へ移し、Git の管理をやめる。ファイルはそのまま残す |
| `delete_project` | プロジェクトのフォルダをゴミ箱へ移し、Revia の登録から外す。`deleteRepository = true` なら、先に GitHub のリポジトリを削除する |

GitHub のリモートとの連携だけを解除する操作は、既存の `remove_remote` を使う（ファイルもコミット履歴も GitHub のリポジトリも残る）。

### 共通の決まり

- 対象は `projectId` で指定し、パスは登録情報から引く。フロントエンドからパスや `owner/name` を受け取らない。
- `fingerprint` は `preview_removal` の値。実行直前に同じ方法で計算し直し、合わなければ何もせず `state_changed`。`fingerprint` は HEAD、現在のブランチ、変更の件数、stash の件数、リモートの URL、どのリモートにもないコミットの数から作る。
- `confirm` は利用者が確認のために入力した文字列で、Rust 側でも照合する。`remove_git` と、`deleteRepository = false` の `delete_project` はフォルダ名（`Project.name`）。`deleteRepository = true` の `delete_project` は `owner/name`。合わなければ何もせず `invalid_input`。
- そのプロジェクトで操作（`start_operation` の項目や単体の変更）が実行中なら `busy`。リポジトリのロックを取ってから実行する。
- 実行後は履歴に記録する（`remove_git`、`delete_local`、`delete_repository`）。`params` には `name`（フォルダ名）と、リポジトリを削除した場合は `repo`（`owner/name`）を入れる。

### `preview_removal`

- `repository` は `.git`（ディレクトリまたはファイル）があるかどうか。リポジトリでなければ、数値は 0、`unpushedCommits` は 0、配列は空、`github` は `null`。
- `unpushedCommits` は、ローカルブランチからたどれて、どのリモート追跡ブランチからもたどれないコミットの数（`git rev-list --count --branches --not --remotes`）。コミットがまだなければ 0。数えられなければ `null`。
- `localOnlyBranches` は、どのリモートにも同じ名前のリモート追跡ブランチがないローカルブランチ（名前順）。
- `github` は `origin` が GitHub を指していればそれ、そうでなければ名前順で最初に見つかった GitHub のリモート。
- `nestedProjects` は、このフォルダの配下にある、ほかの登録済みプロジェクトのパス（パス順）。
- フォルダがない（`missing`）場合は `path_not_found`。

### `check_repository_deletion`

- `github` が `null` なら `invalid_input`。
- `gh` がなければ `blocker = "gh_not_installed"`、未ログインなら `"not_authenticated"`（どちらも `visibility` と `isFork` は `null`）。
- `gh repo view <owner>/<name> --json nameWithOwner,visibility,isFork,viewerCanAdminister` で調べる。見つからなければ `"not_found"`、`viewerCanAdminister` が false なら `"no_permission"`。
- `gh auth status` が示すトークンのスコープに `delete_repo` がなければ `"scope_missing"`。スコープを読み取れない場合（環境変数のトークンなど）は妨げにしない。
- 接続できない場合などは、通常どおりエラーで返す（`network_unreachable` など）。

### `remove_git`

- リポジトリでなければ `not_a_repository`。`.git` がディレクトリでない場合（リンクされた作業ツリーやサブモジュール）は `invalid_input`（`detail` に理由）。
- `.git` をゴミ箱へ移す。成功したら状態を取り直し（`not_initialized` になる）、`revia:project-updated` を送って `Project` を返す。保存済みの `branchTargets`、`defaultRemote`、`defaultBranch` は消す。

### `delete_project`

- 次の場合は何もせず `invalid_path`: フォルダがシンボリックリンク、ホームディレクトリそのものかその上位、ファイルシステムのルート、Revia の設定・データ・キャッシュのフォルダを含む。
- `nestedProjects` が空でなければ `invalid_input`（`detail` に件数）。中にほかのプロジェクトがあるフォルダは消さない。
- `deleteRepository = true` のとき: `github` が `null` なら `invalid_input`。先に `gh repo delete <owner>/<name> --yes` を実行する。失敗したら、フォルダには触れずにエラーを返す。分類は、スコープ不足（出力に `delete_repo` を含む）が `scope_missing`、権限なし（HTTP 403）が `permission_denied`、見つからない（HTTP 404）が `remote_not_found`、未ログインが `not_authenticated`、`gh` なしが `gh_not_installed`。
- 次にフォルダをゴミ箱へ移す。失敗した場合、リポジトリを削除していなければエラーを返す。リポジトリを削除済みなら、成功したように見せないために拒否せず、`RemovalResult` を `repository = "deleted"`、`local = "failed"`、`error` 付きで返す（登録はそのまま残す）。
- フォルダを移せたら、登録から外す: 手動追加の一覧、`ProjectSettings`、状態のキャッシュ。そのフォルダがスキャンルートそのものなら、ルートも外す。`excludedPaths` には加えない。`revia:projects-changed` を送り、`workspace` に最新の一覧を入れて返す。
- フォルダが既にない（`missing`）場合は `path_not_found`（一覧から外すだけなら `exclude_project` を使う）。

### テストのための差し替え

ゴミ箱へ移す処理は、`core::Options` に差し替えられる形で持たせる（本番は OS のゴミ箱、テストは一時フォルダへ移す実装）。テストで利用者の本物のゴミ箱を使わない。`gh` はこれまでどおり模擬の実行ファイルで確かめる。

## 12. 公開前の除外

最初のコミットに入れるべきでないファイル（秘密情報、個人情報が入っていそうなデータ、OS やエディタが作るファイル、依存パッケージ、ビルドの出力、一時ファイル）を見つけ、`.gitignore` に追加してから公開できるようにする。候補を選ぶのは利用者で、バックエンドは候補を挙げることと、渡されたパターンを書き込むことだけを行う。

### `scan_ignore_candidates`

- 引数は `projectId`。最初のコミットに入るはずのファイルを調べて `IgnoreScan` を返す。何も変更しない。ネットワークに出ない。
- 調べる対象は、Git が無視していない未追跡のファイル。リポジトリならそのまま Git に尋ねる。Git 未初期化のフォルダは、プロジェクトの外の一時フォルダに作った git dir と `--work-tree` を使い、既存の `.gitignore` とグローバルの除外設定を Git に判定させる（プロジェクトのフォルダに `.git` を作らない）。追跡済みのファイルは対象外（`.gitignore` に書いても効かないため）。
- 探索の上限は 20,000 エントリ、深さ 12、3 秒。超えたら `truncated = true` にして、そこまでの結果を返す。依存パッケージ・ビルドの出力・キャッシュと判定したフォルダの中には入らず、フォルダ 1 件として扱う。シンボリックリンクはたどらない。
- 内容の検査は 1 ファイル 256KB まで、全体で 2,000 ファイル・32MB まで。NUL を含むファイル（バイナリ）は検査しない。見つけた文字列そのものは、応答・ログ・履歴に含めない（返すのはパスだけ）。
- `pattern` の決まり: 個別のファイルやフォルダは、`/` で始まるルートからのパスにする（`.gitignore` で特別な意味を持つ文字はエスケープする）。名前で決まるもの（`.DS_Store`、`node_modules/`、`*.pyc` など）は、どの階層にも効く形にする。同じ `pattern` は 1 件にまとめ、`matches` に件数、`examples` に該当したパスを最大 5 件入れる。
- 並び順は `category`（`secret`、`personal`、`system`、`dependency`、`build`、`scratch`）、次に `pattern`。
- フォルダがない場合は `path_not_found`。

判定の規則（名前は大文字と小文字を区別しない）:

| 分類 | 根拠 | 対象 | パターン |
| --- | --- | --- | --- |
| `secret` | `env_file` | `.env`、`.env.*`（`.env.example`、`.env.sample`、`.env.template`、`.env.dist` を除く） | ルートからのパス |
| `secret` | `key_file` | `*.pem`、`*.key`、`*.p12`、`*.pfx`、`*.jks`、`*.keystore`、`*.ppk`、`id_rsa*`、`id_dsa*`、`id_ecdsa*`、`id_ed25519*`（`*.pub` を除く） | ルートからのパス |
| `secret` | `credential_file` | `.npmrc`、`.netrc`、`.pypirc`、`.htpasswd`、`credentials.json`、`client_secret*.json`、`service-account*.json`、`*-firebase-adminsdk-*.json`、`secrets.*`、`*.tfvars`、`terraform.tfstate`、`terraform.tfstate.*`、`*.kdbx` | ルートからのパス |
| `secret` | `secret_in_content` | 本文に、秘密鍵のブロック（`-----BEGIN … PRIVATE KEY-----`）か、形の決まったトークン（AWS のアクセスキー `AKIA…`、GitHub の `ghp_` / `gho_` / `ghu_` / `ghs_` / `ghr_` / `github_pat_`、Slack の `xox?-`、Google の `AIza…`、Stripe の `sk_live_`）がある | ルートからのパス |
| `personal` | `personal_data_file` | `*.vcf`。名前に `contacts`、`customers`、`addressbook`、`address_book`、`members`、`顧客`、`名簿`、`住所録`、`連絡先`、`個人情報` のいずれかを含む `.csv` / `.tsv` / `.xlsx` / `.xls` / `.json` / `.sql` / `.txt` / `.numbers` | ルートからのパス |
| `personal` | `personal_in_content` | `.csv` / `.tsv` / `.json` / `.ndjson` / `.sql` / `.txt` / `.xml` / `.yaml` / `.yml` の本文に、異なるメールアドレスが 20 件以上、または電話番号らしい並びが 20 件以上ある | ルートからのパス |
| `personal` | `database_file` | `*.sqlite`、`*.sqlite3`、`*.db`（`Thumbs.db` を除く）、`*.mdb`、`*.accdb`、`*.dump`、`*.sql.gz`、`dump.sql` | ルートからのパス |
| `system` | `os_file` | `.DS_Store`、`Thumbs.db`、`desktop.ini`、`._*` | 名前のまま |
| `system` | `editor_file` | `*.swp`、`*.swo`、`*~`、`.idea/`、`*.iml` | 名前のまま |
| `dependency` | `dependency_dir` | `node_modules/`、`bower_components/`、`.venv/`、`venv/`、`Pods/`、`.gradle/`、`__pypackages__/` | `名前/` |
| `build` | `build_output` | `dist/`、`build/`、`out/`、`target/`、`.next/`、`.nuxt/`、`.output/`、`.svelte-kit/`、`.turbo/`、`DerivedData/`、`coverage/`、`__pycache__/`、`*.pyc` | 名前のまま |
| `build` | `cache_dir` | `.cache/`、`.parcel-cache/`、`.pytest_cache/`、`.mypy_cache/`、`.ruff_cache/`、`.sass-cache/`、`.eslintcache` | 名前のまま |
| `scratch` | `log_file` | `*.log` | `*.log` |
| `scratch` | `temp_file` | `*.tmp`、`*.temp`、`*.bak`、`*.old`、`*.orig`、`tmp/`、`temp/` | 名前のまま |
| `scratch` | `scratch_file` | 拡張子を除いた名前が `test`、`tmp`、`temp`、`scratch`、`aaa`、`hoge`、`fuga`、`piyo` のいずれかで、拡張子がないか `.txt` / `.md` のもの。名前が `untitled`、`無題`、`新規` で始まるもの | ルートからのパス |

テストコード（`tests/` や `__tests__/` の中、`*.test.*`、`*.spec.*`、`test_*.py`、`*_test.go` など）は、`personal` と `scratch` の規則、および本文の検査の対象にしない（テスト用のデータや例示の文字列を候補にしないため）。名前で決まる `secret`、`system`、`dependency`、`build` の規則は、テストのフォルダの中でも適用する。1 つのファイルが複数の規則に当たる場合は、表で上にある規則を使う。

### 公開での適用（`PublishItem.ignore`）

- 使うのは、その項目で最初のコミットを作る場合（コミットがまだなく、`initialCommitMessage` がある）だけ。それ以外では無視し、手順にも入れない。
- パターンは検査する。空、300 文字超、制御文字や改行を含む、先頭に空白がある、末尾に空白がある（`\ ` でエスケープした空白を除く）、`!` か `#` で始まる、`..` の要素を含む、のいずれかがあれば `invalid_input`。1 項目あたり 500 件まで。
- 手順 `ignore` は `init` のあと、`initial_commit` の前に実行する。プロジェクト直下の `.gitignore` に、まだない行だけを追記する（なければ作る）。既存の内容は変えない。追記の前に `# Added by Revia` の行を 1 行入れる。`.gitignore` がシンボリックリンクや通常のファイル以外なら `failed`（`invalid_path`）。
- `plan_operation` は、書き足す行数を `{ code: "ignore", patterns }` で返し（0 なら手順に入れない）、`initial_commit.files` と `sensitive_files` / `large_files` / `many_files` の警告を、パターンを適用したあとのファイルで求める。`ignore` があるときは `no_gitignore` を出さない。
- 途中で失敗・中止した場合も、書き込んだ `.gitignore` は戻さない（`completed` に `ignore` を入れる）。

