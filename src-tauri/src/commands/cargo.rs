use std::path::Path;

use crate::adapters::{cargo::CargoAdapter, ConfigAdapter};
use crate::domain::{ReadResult, ToolContext};

#[tauri::command]
pub fn scan_cargo(project_directory: Option<String>) -> Result<ReadResult, String> {
    let project_directory = project_directory
        .filter(|path| !path.trim().is_empty())
        .map(validate_project_directory)
        .transpose()?;

    CargoAdapter::from_system()
        .read(&ToolContext {
            project_directory,
            include_project_sources: true,
        })
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
