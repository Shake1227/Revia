use crate::{
    core::Core,
    error::{AppError, Result},
    settings,
};
use serde_json::{Value, json};
impl Core {
    pub async fn history_record(
        &self,
        operation: Option<&str>,
        id: &str,
        action: &str,
        outcome: &str,
        error: Option<&AppError>,
    ) -> Result<()> {
        self.history_record_params(operation, id, action, outcome, error, json!({}))
            .await
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn history_record_params(
        &self,
        operation: Option<&str>,
        id: &str,
        action: &str,
        outcome: &str,
        error: Option<&AppError>,
        params: Value,
    ) -> Result<()> {
        let project = if id.is_empty() && action == "clone" {
            json!({"name":"Clone","status":null})
        } else {
            self.project(id)?
        };
        self.history_record_snapshot(operation, id, action, outcome, error, params, &project)
            .await
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn history_record_snapshot(
        &self,
        operation: Option<&str>,
        id: &str,
        action: &str,
        outcome: &str,
        error: Option<&AppError>,
        mut params: Value,
        project: &Value,
    ) -> Result<()> {
        let _guard = self.settings_lock.lock().await;
        if params.get("branch").is_none() {
            params["branch"] = project["status"]["branch"].clone();
        }
        fn redact(v: &mut Value) {
            match v {
                Value::String(s) => *s = crate::error::sanitize(s),
                Value::Array(a) => a.iter_mut().for_each(redact),
                Value::Object(o) => o.values_mut().for_each(redact),
                _ => {}
            }
        }
        redact(&mut params);
        if let Some(params) = params.as_object_mut() {
            params.retain(|_, v| !v.is_null());
        }
        let mut records = self.history_read();
        records.push(json!({"id":uuid::Uuid::new_v4().to_string(),"at":crate::now(),"operationId":operation,"projectId":if id.is_empty() {Value::Null} else {json!(id)},"projectName":crate::error::sanitize(project["name"].as_str().unwrap_or("")),"action":action,"outcome":outcome,"params":params,"errorCode":error.map(|e|&e.code)}));
        if records.len() > 2000 {
            records.drain(..records.len() - 2000);
        }
        let mut bytes = vec![];
        for r in records {
            serde_json::to_writer(&mut bytes, &r)?;
            bytes.push(b'\n');
        }
        settings::atomic(&self.options.data_dir, "history.jsonl", &bytes)
    }
    fn history_read(&self) -> Vec<Value> {
        std::fs::read_to_string(self.options.data_dir.join("history.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|s| serde_json::from_str(s).ok())
            .collect()
    }
    pub async fn history(
        &self,
        limit: usize,
        before: Option<String>,
        project: Option<String>,
    ) -> Result<Vec<Value>> {
        if !(1..=2000).contains(&limit) {
            return Err(AppError::new("invalid_input"));
        }
        let _guard = self.settings_lock.lock().await;
        let rows = self.history_read();
        let mut rows = rows.into_iter().rev();
        if let Some(id) = before
            && !rows.by_ref().any(|r| r["id"] == id)
        {
            return Ok(vec![]);
        }
        Ok(rows
            .filter(|r| project.as_ref().is_none_or(|id| r["projectId"] == *id))
            .take(limit)
            .collect())
    }
    pub async fn clear_history(&self) -> Result<()> {
        let _guard = self.settings_lock.lock().await;
        settings::atomic(&self.options.data_dir, "history.jsonl", b"")
    }
}
