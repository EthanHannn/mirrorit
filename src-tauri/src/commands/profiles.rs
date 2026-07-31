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

#[tauri::command]
pub fn export_npm_profile(
    request: ExportNpmProfileRequest,
) -> Result<ProfileExportDocument, String> {
    build_npm_export(request.profile)
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
}
