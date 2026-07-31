use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::adapters::{flutter_pub::FlutterPubAdapter, ConfigAdapter, PlanRequest};
use crate::domain::{
    ApplyResult, ChangePlan, NonSensitiveValue, Operation, Profile, ReadResult, SnapshotRef,
    ToolContext,
};

#[derive(Default)]
pub struct FlutterPubPreviewStore(pub Mutex<BTreeMap<String, ChangePlan>>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlutterPubHostedPreviewRequest {
    pub hosted_url: Option<String>,
}

#[tauri::command]
pub fn scan_flutter_pub(project_directory: Option<String>) -> Result<ReadResult, String> {
    let project_directory = project_directory
        .filter(|path| !path.trim().is_empty())
        .map(validate_project_directory)
        .transpose()?;

    FlutterPubAdapter::from_system()
        .read(&ToolContext {
            project_directory,
            include_project_sources: true,
        })
        .map_err(|error| error.message)
}

#[tauri::command]
pub fn preview_flutter_pub_hosted_update(
    request: FlutterPubHostedPreviewRequest,
    preview_store: tauri::State<'_, FlutterPubPreviewStore>,
) -> Result<ChangePlan, String> {
    let hosted_url = request.hosted_url.map(|value| value.trim().to_owned());
    if hosted_url.as_deref().is_some_and(str::is_empty) {
        return Err("自定义 hosted 源不能为空；恢复官方源请使用官方配置档。".into());
    }
    let adapter = FlutterPubAdapter::from_system();
    let current_config = adapter
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .map_err(|error| error.message)?;
    let profile = Profile {
        id: hosted_url
            .as_ref()
            .map(|_| "flutter-pub-custom")
            .unwrap_or("flutter-pub-official")
            .into(),
        name: "Flutter/Pub hosted 源".into(),
        values: BTreeMap::from([(
            "hosted.default".into(),
            NonSensitiveValue::new(hosted_url.unwrap_or_default()),
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
        .map_err(|_| "无法保存 Flutter/Pub 预览，请重试。".to_owned())?
        .insert(plan.id.clone(), plan.clone());
    Ok(plan)
}

#[tauri::command]
pub fn apply_flutter_pub_preview(
    plan_id: String,
    preview_store: tauri::State<'_, FlutterPubPreviewStore>,
) -> Result<ApplyResult, String> {
    let plan = preview_store
        .0
        .lock()
        .map_err(|_| "无法读取 Flutter/Pub 预览，请重新预览。".to_owned())?
        .remove(&plan_id)
        .ok_or_else(|| "Flutter/Pub 预览已失效，请重新预览。".to_owned())?;
    let confirmed_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "系统时间不可用，无法确认 Flutter/Pub 操作。".to_owned())?
        .as_millis() as i64;

    FlutterPubAdapter::from_system()
        .apply(plan.confirm(confirmed_at_ms))
        .map_err(|error| error.message)
}

#[tauri::command]
pub fn rollback_flutter_pub_snapshot(snapshot_id: String) -> Result<Operation, String> {
    FlutterPubAdapter::from_system()
        .rollback(&SnapshotRef(snapshot_id))
        .map_err(|error| error.message)
}

fn validate_project_directory(path: String) -> Result<String, String> {
    let path = Path::new(&path);
    if !path.is_dir() {
        return Err("项目目录不存在或不是文件夹。".into());
    }
    path.canonicalize()
        .map(|canonical_path| canonical_path.display().to_string())
        .map_err(|error| format!("无法读取项目目录：{error}"))
}
