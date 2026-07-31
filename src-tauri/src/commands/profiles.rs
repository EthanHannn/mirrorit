use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::ToolId;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportNpmProfileRequest {
    pub profile: ExportNpmProfileInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportNpmProfileInput {
    pub id: String,
    pub name: String,
    pub registry: String,
}

#[derive(Debug, Serialize)]
pub struct ProfileExportDocument {
    pub format: &'static str,
    pub version: u32,
    pub profiles: Vec<ExportedProfile>,
}

#[derive(Debug, Serialize)]
pub struct ExportedProfile {
    pub tool: ToolId,
    pub id: String,
    pub name: String,
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewNpmProfileImportRequest {
    pub content: String,
    pub current_registry: String,
}

#[derive(Debug, Deserialize)]
struct ImportedProfileDocument {
    format: String,
    version: u32,
    profiles: Vec<ImportedProfile>,
}

#[derive(Debug, Deserialize)]
struct ImportedProfile {
    tool: ToolId,
    id: String,
    name: String,
    values: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct NpmProfileImportPreview {
    pub id: String,
    pub name: String,
    pub current_registry: String,
    pub imported_registry: String,
    pub changed: bool,
}

#[tauri::command]
pub fn export_npm_profile(
    request: ExportNpmProfileRequest,
) -> Result<ProfileExportDocument, String> {
    build_npm_export(request.profile)
}

#[tauri::command]
pub fn preview_npm_profile_import(
    request: PreviewNpmProfileImportRequest,
) -> Result<NpmProfileImportPreview, String> {
    build_npm_import_preview(&request.content, &request.current_registry)
}

fn build_npm_export(profile: ExportNpmProfileInput) -> Result<ProfileExportDocument, String> {
    if profile.id.trim().is_empty() || profile.name.trim().is_empty() {
        return Err("配置档必须包含名称和标识。".into());
    }
    let registry = profile.registry.trim();
    if !registry.starts_with("https://")
        || registry[8..].contains('@')
        || registry.contains(char::is_whitespace)
    {
        return Err("配置档包含不安全或无效的 registry 地址。".into());
    }

    Ok(ProfileExportDocument {
        format: "mirrorit-profile",
        version: 1,
        profiles: vec![ExportedProfile {
            tool: ToolId::Npm,
            id: profile.id.trim().to_owned(),
            name: profile.name.trim().to_owned(),
            values: BTreeMap::from([("registry".into(), registry.to_owned())]),
        }],
    })
}

fn build_npm_import_preview(
    content: &str,
    current_registry: &str,
) -> Result<NpmProfileImportPreview, String> {
    let document = serde_json::from_str::<ImportedProfileDocument>(content)
        .map_err(|_| "导入文件不是有效的 MirrorIt JSON 配置档。".to_owned())?;
    if document.format != "mirrorit-profile" || document.version != 1 {
        return Err("导入文件格式或版本不受支持。".into());
    }
    let [profile] = document.profiles.as_slice() else {
        return Err("导入文件必须只包含一个 npm 配置档。".into());
    };
    if profile.tool != ToolId::Npm
        || profile.id.trim().is_empty()
        || profile.name.trim().is_empty()
        || profile.values.len() != 1
    {
        return Err("导入文件包含不受支持的配置档内容。".into());
    }
    let Some(registry) = profile.values.get("registry") else {
        return Err("导入文件包含不受支持的配置档内容。".into());
    };
    validate_registry(registry)?;
    validate_registry(current_registry)?;

    Ok(NpmProfileImportPreview {
        id: profile.id.trim().to_owned(),
        name: profile.name.trim().to_owned(),
        current_registry: current_registry.trim().to_owned(),
        imported_registry: registry.trim().to_owned(),
        changed: current_registry.trim() != registry.trim(),
    })
}

fn validate_registry(registry: &str) -> Result<(), String> {
    let registry = registry.trim();
    if !registry.starts_with("https://")
        || registry[8..].contains('@')
        || registry.contains(char::is_whitespace)
    {
        return Err("配置档包含不安全或无效的 registry 地址。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_a_deterministic_non_sensitive_document() {
        let document = build_npm_export(ExportNpmProfileInput {
            id: "npm-official".into(),
            name: "官方源".into(),
            registry: "https://registry.npmjs.org/".into(),
        })
        .expect("export document");

        assert_eq!(document.format, "mirrorit-profile");
        assert_eq!(document.version, 1);
        assert_eq!(document.profiles[0].tool, ToolId::Npm);
        assert_eq!(
            document.profiles[0].values["registry"],
            "https://registry.npmjs.org/"
        );
    }

    #[test]
    fn rejects_a_registry_with_embedded_credentials() {
        let error = build_npm_export(ExportNpmProfileInput {
            id: "custom".into(),
            name: "自定义源".into(),
            registry: "https://alice:secret@registry.example/".into(),
        })
        .expect_err("credential URL should be rejected");

        assert_eq!(error, "配置档包含不安全或无效的 registry 地址。");
    }

    #[test]
    fn previews_a_valid_export_without_writing() {
        let preview = build_npm_import_preview(
            r#"{"format":"mirrorit-profile","version":1,"profiles":[{"tool":"npm","id":"custom","name":"自定义源","values":{"registry":"https://registry.example/"}}]}"#,
            "https://registry.npmjs.org/",
        )
        .expect("import preview");

        assert!(preview.changed);
        assert_eq!(preview.imported_registry, "https://registry.example/");
    }

    #[test]
    fn rejects_unknown_fields_and_versions() {
        let error = build_npm_import_preview(
            r#"{"format":"mirrorit-profile","version":2,"profiles":[{"tool":"npm","id":"custom","name":"自定义源","values":{"registry":"https://registry.example/","token":"secret"}}]}"#,
            "https://registry.npmjs.org/",
        )
        .expect_err("unsupported import should be rejected");

        assert_eq!(error, "导入文件格式或版本不受支持。");
    }
}
