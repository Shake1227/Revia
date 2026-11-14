use crate::{
    core::Core,
    details::resolve,
    error::{AppError, Result},
    status,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
impl Core {
    pub(crate) async fn preview_fingerprint(
        &self,
        p: &Value,
        refs: &[(&str, &str)],
    ) -> Result<String> {
        let root = Path::new(p["path"].as_str().unwrap());
        let mut data =
            json!({"branch":p["status"]["branch"],"targets":p["status"]["targets"],"refs":refs});
        data["head"] = json!(resolve(&self.options.git, root, "HEAD").await.ok());
        data["status"] = json!(
            self.options
                .git
                .local(
                    root,
                    &["status", "--porcelain=v2", "-z", "--untracked-files=all"]
                )
                .await?
        );
        data["index"] = json!(
            self.options
                .git
                .local(root, &["ls-files", "--stage", "-z"])
                .await?
        );
        data["diff"] = json!(
            self.options
                .git
                .local(
                    root,
                    &[
                        "diff",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--binary",
                        "HEAD",
                        "--"
                    ]
                )
                .await
                .ok()
        );
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&data)?)))
    }
    async fn preview_project(&self, id: &str) -> Result<Value> {
        self.refresh(Some(vec![id.into()])).await?;
        let p = self.project(id)?;
        if matches!(
            p["status"]["state"].as_str(),
            Some("missing" | "not_initialized" | "unknown")
        ) {
            return Err(AppError::new("not_a_repository"));
        }
        Ok(p)
    }
    pub(crate) fn local_blocker(p: &Value, dirty: bool) -> Option<&'static str> {
        let s = &p["status"];
        if s["unborn"] == true {
            Some("unborn")
        } else if s["detached"] == true {
            Some("detached")
        } else if s["changes"]["conflicted"].as_u64().unwrap_or(0) > 0 {
            Some("conflicted")
        } else if !s["inProgress"].is_null() {
            Some("in_progress")
        } else if dirty && s["changes"]["total"].as_u64().unwrap_or(0) > 0 {
            Some("dirty_worktree")
        } else {
            None
        }
    }
    async fn range_commits(&self, id: &str, base: &str, tip: &str) -> Result<(Vec<Value>, u64)> {
        let p = self.project(id)?;
        let root = Path::new(p["path"].as_str().unwrap());
        let range = format!("{base}..{tip}");
        let commits = self.commit_log_resolved(id, &range, 50, 0).await?;
        Ok((
            commits,
            status::count(&self.options.git, root, &range)
                .await
                .unwrap_or(0),
        ))
    }
    pub async fn preview_merge(&self, id: &str, source: &str, target: &str) -> Result<Value> {
        let p = self.preview_project(id).await?;
        let root = Path::new(p["path"].as_str().unwrap());
        let g = &self.options.git;
        g.reference(root, target).await?;
        let source_sha = resolve(g, root, source).await?;
        let target_sha = resolve(g, root, &format!("refs/heads/{target}"))
            .await
            .map_err(|_| AppError::new("branch_not_found"))?;
        let up_to_date = g
            .local(
                root,
                &["merge-base", "--is-ancestor", &source_sha, &target_sha],
            )
            .await
            .is_ok();
        let ff = g
            .local(
                root,
                &["merge-base", "--is-ancestor", &target_sha, &source_sha],
            )
            .await
            .is_ok();
        let (commits, count) = self.range_commits(id, &target_sha, &source_sha).await?;
        let changed = g
            .local(
                root,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--name-only",
                    "-z",
                    &target_sha,
                    &source_sha,
                    "--",
                ],
            )
            .await?;
        let conflicts = if up_to_date || ff {
            json!({"status":"none","paths":[]})
        } else {
            g.merge_conflicts(root, &target_sha, &source_sha).await
        };
        let dirty = p["status"]["changes"]["total"].as_u64().unwrap_or(0) > 0;
        Ok(
            json!({"source":{"name":source,"sha":source_sha},"target":{"name":target,"sha":target_sha},"needsCheckout":p["status"]["branch"]!=target,"worktreeDirty":dirty,"upToDate":up_to_date,"fastForward":ff,"commits":commits,"commitCount":count,"filesChanged":changed.split('\0').filter(|s|!s.is_empty()).count(),"conflicts":conflicts,"blocker":Self::local_blocker(&p,true),"fingerprint":self.preview_fingerprint(&p,&[("source",&source_sha),("target",&target_sha)]).await?}),
        )
    }
    pub async fn preview_rebase(&self, id: &str, onto: &str) -> Result<Value> {
        let p = self.preview_project(id).await?;
        let root = Path::new(p["path"].as_str().unwrap());
        let sha = resolve(&self.options.git, root, onto).await?;
        let (commits, count) = self.range_commits(id, &sha, "HEAD").await?;
        let local = self
            .options
            .git
            .local(root, &["rev-list", &format!("{sha}..HEAD")])
            .await?;
        let published = self
            .options
            .git
            .local(root, &["rev-list", "--remotes"])
            .await
            .unwrap_or_default();
        let set = published.lines().collect::<BTreeSet<_>>();
        let published = local.lines().filter(|s| set.contains(s)).count();
        Ok(
            json!({"onto":{"name":onto,"sha":sha},"branch":p["status"]["branch"].as_str().unwrap_or(""),"commits":commits,"commitCount":count,"publishedCount":published,"worktreeDirty":p["status"]["changes"]["total"].as_u64().unwrap_or(0)>0,"blocker":Self::local_blocker(&p,true),"fingerprint":self.preview_fingerprint(&p,&[("onto",&sha)]).await?}),
        )
    }
    pub async fn preview_reset(&self, id: &str, target: &str, mode: &str) -> Result<Value> {
        if !matches!(mode, "soft" | "mixed" | "hard") {
            return Err(AppError::new("invalid_input"));
        }
        let p = self.preview_project(id).await?;
        let root = Path::new(p["path"].as_str().unwrap());
        let sha = resolve(&self.options.git, root, target).await?;
        let (commits, count) = self.range_commits(id, &sha, "HEAD").await?;
        let lost = self
            .options
            .git
            .local(
                root,
                &["rev-list", &format!("{sha}..HEAD"), "--not", "--remotes"],
            )
            .await?;
        let blocker = if mode == "hard" && !self.hard_reset_allowed() {
            Some("feature_disabled")
        } else {
            Self::local_blocker(&p, false)
        };
        Ok(
            json!({"target":{"name":target,"sha":sha},"branch":p["status"]["branch"].as_str().unwrap_or(""),"mode":mode,"commits":commits,"commitCount":count,"unpushedCount":lost.lines().count(),"discardedFiles":if mode=="hard"{p["status"]["changes"]["total"].as_u64().unwrap_or(0)}else{0},"blocker":blocker,"fingerprint":self.preview_fingerprint(&p,&[("target",&sha),("mode",mode)]).await?}),
        )
    }
    pub(crate) fn hard_reset_allowed(&self) -> bool {
        self.state.lock().unwrap().settings.settings["general"]["safety"]["allowHardReset"] == true
    }
}
