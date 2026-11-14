import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

const [dump, typesModule] = process.argv.slice(2);
const read = (name) => readFileSync(join(dump, `${name}.json`), "utf8");

const samples = [
  ["workspace", "Workspace"],
  ["workspace_after", "Workspace"],
  ["app_state", "AppState"],
  ["details", "ProjectDetail[]"],
  ["changes", "ChangedFile[][]"],
  ["diffs", "FileDiff[]"],
  ["logs", "CommitInfo[][]"],
  ["commit_details", "CommitDetail[]"],
  ["conflicts", "ConflictState[]"],
  ["plans", "OperationPlan[]"],
  ["merge_preview", "MergePreview"],
  ["rebase_preview", "RebasePreview"],
  ["reset_preview", "ResetPreview"],
  ["operations", "OperationSnapshot[]"],
  ["history", "HistoryEntry[]"],
  ["clone_operation", "OperationSnapshot"],
  ["ignore_scan", "IgnoreScan"],
  ["removal_previews", "RemovalPreview[]"],
  ["removed_git", "Project"],
  ["removal_result", "RemovalResult"],
  ["update_infos", "UpdateInfo[]"],
  ["update_info_none", "UpdateInfo"],
  ["github_status", "GithubStatus"],
  ["github_owners", "GithubOwner[]"],
  ["github_repos", "GithubRepo[]"],
];

console.log(`import type * as T from ${JSON.stringify(typesModule)};`);
for (const [name, type] of samples) {
  if (!existsSync(join(dump, `${name}.json`))) continue;
  const annotated = type.replace(/[A-Z][A-Za-z]+/g, (match) => `T.${match}`);
  console.log(`export const ${name}: ${annotated} = ${read(name)};`);
}
