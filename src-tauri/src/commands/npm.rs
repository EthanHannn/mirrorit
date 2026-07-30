use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

use serde::Deserialize;

use crate::adapters::{npm::NpmAdapter, ConfigAdapter};
use crate::domain::{
    ApplyResult, ChangePlan, ConfigScope, NonSensitiveValue, Operation, Profile, ReadResult,
    SnapshotRef, ToolContext,
};

#[derive(Default)]
pub struct NpmPreviewStore(Mutex<BTreeMap<String, ChangePlan>>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpmTargetScope {
    User,
    Project,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpmProfileInput {
    pub id: String,
    pub name: String,
    pub registry: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NpmPreviewRequest {
    pub project_directory: Option<String>,
    pub target_scope: NpmTargetScope,
    pub profile: NpmProfileInput,
}

#[tauri::command]
pub fn scan_npm(project_directory: Option<String>) -> Result<ReadResult, String> {
    let project_directory = project_directory
        .filter(|path| !path.trim().is_empty())
        .map(validate_project_directory)
        .transpose()?;

    NpmAdapter::from_system()
        .read(&ToolContext {
            project_directory,
            include_project_sources: true,
        })
        .map_err(|error| error.message)
}

#[tauri::command]
pub fn preview_npm_profile(
    request: NpmPreviewRequest,
    preview_store: tauri::State<'_, NpmPreviewStore>,
) -> Result<ChangePlan, String> {
    let project_directory = request
        .project_directory
        .filter(|path| !path.trim().is_empty())
        .map(validate_project_directory)
        .transpose()?;
    validate_profile(&request.profile)?;

    let adapter = NpmAdapter::from_system();
    let current_config = adapter
        .read(&ToolContext {
            project_directory: project_directory.clone(),
            include_project_sources: true,
        })
        .map_err(|error| error.message)?;
    let (path, scope, priority) = match request.target_scope {
        NpmTargetScope::User => user_target_path()?,
        NpmTargetScope::Project => project_target_path(project_directory)?,
    };
    let profile = Profile {
        id: request.profile.id,
        name: request.profile.name,
        values: std::collections::BTreeMap::from([(
            "registry".into(),
            NonSensitiveValue::new(request.profile.registry),
        )]),
    };

    let plan = adapter
        .plan_for_target(&path, scope, priority, &profile, &current_config)
        .map_err(|error| error.message)?;
    preview_store
        .0
        .lock()
        .map_err(|_| "无法保存本次预览，请重试。".to_owned())?
        .insert(plan.id.clone(), plan.clone());

    Ok(plan)
}

#[tauri::command]
pub fn apply_npm_preview(
    plan_id: String,
    preview_store: tauri::State<'_, NpmPreviewStore>,
) -> Result<ApplyResult, String> {
    let plan = preview_store
        .0
        .lock()
        .map_err(|_| "无法读取预览，请重新预览。".to_owned())?
        .remove(&plan_id)
        .ok_or_else(|| "预览已失效，请重新预览。".to_owned())?;
    let confirmed_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "系统时间不可用，无法确认操作。".to_owned())?
        .as_millis() as i64;

    NpmAdapter::from_system()
        .apply(plan.confirm(confirmed_at_ms))
        .map_err(|error| error.message)
}

#[tauri::command]
pub fn rollback_npm_snapshot(snapshot_id: String) -> Result<Operation, String> {
    NpmAdapter::from_system()
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

fn user_target_path() -> Result<(std::path::PathBuf, ConfigScope, u32), String> {
    let directory = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "未能定位用户目录，无法预览用户级配置。".to_owned())?;

    Ok((Path::new(&directory).join(".npmrc"), ConfigScope::User, 100))
}

fn project_target_path(
    project_directory: Option<String>,
) -> Result<(std::path::PathBuf, ConfigScope, u32), String> {
    let Some(project_directory) = project_directory else {
        return Err("预览项目级配置时必须填写项目目录。".into());
    };

    Ok((
        Path::new(&project_directory).join(".npmrc"),
        ConfigScope::Project,
        200,
    ))
}

fn validate_profile(profile: &NpmProfileInput) -> Result<(), String> {
    if profile.id.trim().is_empty() || profile.name.trim().is_empty() {
        return Err("配置档必须包含名称和标识。".into());
    }

    let registry = profile.registry.trim();
    if !registry.starts_with("https://")
        || registry[8..].contains('@')
        || registry.contains(char::is_whitespace)
    {
        return Err("registry 必须是未含凭据的 HTTPS 地址。".into());
    }

    Ok(())
}
