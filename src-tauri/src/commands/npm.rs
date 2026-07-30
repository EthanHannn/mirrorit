use std::path::Path;

use serde::Deserialize;

use crate::adapters::{npm::NpmAdapter, ConfigAdapter};
use crate::domain::{ConfigScope, NonSensitiveValue, Profile, ReadResult, ToolContext};

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
) -> Result<crate::domain::ChangePlan, String> {
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

    adapter
        .plan_for_target(&path, scope, priority, &profile, &current_config)
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
