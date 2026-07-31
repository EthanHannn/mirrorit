use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::adapters::{maven::MavenAdapter, ConfigAdapter, PlanRequest};
use crate::domain::{
    ApplyResult, ChangePlan, NonSensitiveValue, Operation, Profile, ReadResult, SnapshotRef,
    ToolContext,
};

#[derive(Default)]
pub struct MavenPreviewStore(pub Mutex<BTreeMap<String, ChangePlan>>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MavenMirrorPreviewRequest {
    pub mirror_id: String,
    pub url: String,
}

#[tauri::command]
pub fn scan_maven() -> Result<ReadResult, String> {
    MavenAdapter::from_system()
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .map_err(|error| error.message)
}

#[tauri::command]
pub fn preview_maven_mirror_update(
    request: MavenMirrorPreviewRequest,
    preview_store: tauri::State<'_, MavenPreviewStore>,
) -> Result<ChangePlan, String> {
    validate_request(&request)?;

    let adapter = MavenAdapter::from_system();
    let current_config = adapter
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .map_err(|error| error.message)?;
    let profile = Profile {
        id: format!("maven-mirror-{}", request.mirror_id.trim()),
        name: "Maven mirror URL".into(),
        values: BTreeMap::from([(
            format!("mirror.{}", request.mirror_id.trim()),
            NonSensitiveValue::new(request.url.trim()),
        )]),
    };
    let plan = adapter
        .plan(PlanRequest {
            profile: &profile,
            current_config: &current_config,
        })
        .map_err(|error| error.message)?;
    preview_store
        .0
        .lock()
        .map_err(|_| "无法保存 Maven 预览，请重试。".to_owned())?
        .insert(plan.id.clone(), plan.clone());

    Ok(plan)
}

#[tauri::command]
pub fn apply_maven_preview(
    plan_id: String,
    preview_store: tauri::State<'_, MavenPreviewStore>,
) -> Result<ApplyResult, String> {
    let plan = preview_store
        .0
        .lock()
        .map_err(|_| "无法读取 Maven 预览，请重新预览。".to_owned())?
        .remove(&plan_id)
        .ok_or_else(|| "Maven 预览已失效，请重新预览。".to_owned())?;
    let confirmed_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间不可用，无法确认 Maven 操作。".to_owned())?
        .as_millis() as i64;

    MavenAdapter::from_system()
        .apply(plan.confirm(confirmed_at_ms))
        .map_err(|error| error.message)
}

#[tauri::command]
pub fn rollback_maven_snapshot(snapshot_id: String) -> Result<Operation, String> {
    MavenAdapter::from_system()
        .rollback(&SnapshotRef(snapshot_id))
        .map_err(|error| error.message)
}

fn validate_request(request: &MavenMirrorPreviewRequest) -> Result<(), String> {
    if request.mirror_id.trim().is_empty() {
        return Err("请选择要更新的 Maven 镜像。".into());
    }
    let url = request.url.trim();
    if !url.starts_with("https://") || url[8..].contains('@') || url.contains(char::is_whitespace) {
        return Err("镜像 URL 必须是未含凭据的 HTTPS 地址。".into());
    }

    Ok(())
}
