use crate::adapters::{docker::DockerAdapter, ConfigAdapter};
use crate::domain::{ReadResult, ToolContext};

#[tauri::command]
pub fn scan_docker() -> Result<ReadResult, String> {
    DockerAdapter::from_system()
        .read(&ToolContext {
            project_directory: None,
            include_project_sources: false,
        })
        .map_err(|error| error.message)
}
