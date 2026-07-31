use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_yaml::{Mapping, Value};

use crate::adapters::{AdapterResult, ConfigAdapter, PlanRequest};
use crate::domain::{
    AdapterError, AdapterErrorCode, ApplyResult, ChangePlan, ConfigScope, ConfigSource,
    ConfirmedChangePlan, DetectionContext, EffectiveConfig, EffectiveValue, HealthCheckResult,
    HealthCheckTarget, Operation, ReadResult, SnapshotRef, ToolCapability, ToolContext,
    ToolDetection, ToolId,
};

const DEFAULT_HOSTED_URL: &str = "https://pub.dev";
const SYSTEM_PRIORITY: u32 = 100;
const PROJECT_PRIORITY: u32 = 200;
const ENVIRONMENT_PRIORITY: u32 = 300;

#[derive(Debug, Clone)]
pub struct FlutterPubAdapter {
    environment: BTreeMap<String, String>,
}

impl FlutterPubAdapter {
    pub fn from_system() -> Self {
        Self {
            environment: std::env::vars().collect(),
        }
    }

    #[cfg(test)]
    fn with_environment(environment: BTreeMap<String, String>) -> Self {
        Self { environment }
    }

    fn read_project_file(
        &self,
        path: &Path,
        values: &mut BTreeMap<String, EffectiveValue>,
        diagnostics: &mut Vec<String>,
    ) {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                diagnostics.push(format!("无法读取 {}：{error}", path.display()));
                return;
            }
        };
        let root = match serde_yaml::from_str::<Value>(&content) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(format!("{} YAML 无法解析：{error}", path.display()));
                return;
            }
        };
        let Some(root) = root.as_mapping() else {
            diagnostics.push(format!("{} 顶层必须是 YAML 映射。", path.display()));
            return;
        };
        let location = path.display().to_string();

        if let Some(value) = mapping_value(root, "publish_to").and_then(Value::as_str) {
            add_value(
                values,
                "publish_to".into(),
                redact_credentials(value),
                ConfigScope::Project,
                &location,
                PROJECT_PRIORITY,
            );
        }

        for section in ["dependencies", "dev_dependencies", "dependency_overrides"] {
            let Some(dependencies) = mapping_value(root, section).and_then(Value::as_mapping)
            else {
                continue;
            };
            for (name, definition) in dependencies {
                let Some(name) = name.as_str() else {
                    diagnostics.push(format!(
                        "{} 的 {section} 包含无法识别的依赖名称。",
                        path.display()
                    ));
                    continue;
                };
                match hosted_url(definition) {
                    Ok(Some(url)) => add_value(
                        values,
                        format!("dependency.{section}.{name}.hosted"),
                        redact_credentials(&url),
                        ConfigScope::Project,
                        &location,
                        PROJECT_PRIORITY,
                    ),
                    Ok(None) => {}
                    Err(message) => diagnostics
                        .push(format!("{} 的 {section}.{name} {message}", path.display())),
                }
            }
        }
    }
}

impl ConfigAdapter for FlutterPubAdapter {
    fn tool(&self) -> ToolId {
        ToolId::FlutterPub
    }

    fn detect(&self, _context: &DetectionContext) -> AdapterResult<ToolDetection> {
        let version = ["flutter", "dart"].into_iter().find_map(command_version);
        Ok(ToolDetection {
            tool: self.tool(),
            installed: version.is_some(),
            version,
            capabilities: vec![ToolCapability::Read],
        })
    }

    fn read(&self, context: &ToolContext) -> AdapterResult<ReadResult> {
        let mut values = BTreeMap::new();
        let mut diagnostics = Vec::new();
        add_value(
            &mut values,
            "hosted.default".into(),
            DEFAULT_HOSTED_URL.into(),
            ConfigScope::System,
            "Dart Pub 默认 hosted 源",
            SYSTEM_PRIORITY,
        );

        if let Some((name, value)) = environment_value(&self.environment, "PUB_HOSTED_URL") {
            add_value(
                &mut values,
                "hosted.default".into(),
                redact_credentials(value),
                ConfigScope::Environment,
                name,
                ENVIRONMENT_PRIORITY,
            );
        }
        for (field, name) in [
            ("proxy.http", "HTTP_PROXY"),
            ("proxy.https", "HTTPS_PROXY"),
            ("proxy.no_proxy", "NO_PROXY"),
        ] {
            if let Some((actual_name, value)) = environment_value(&self.environment, name) {
                add_value(
                    &mut values,
                    field.into(),
                    redact_credentials(value),
                    ConfigScope::Environment,
                    actual_name,
                    ENVIRONMENT_PRIORITY,
                );
            }
        }

        if context.include_project_sources {
            if let Some(directory) = &context.project_directory {
                self.read_project_file(
                    &Path::new(directory).join("pubspec.yaml"),
                    &mut values,
                    &mut diagnostics,
                );
            }
        }

        Ok(ReadResult {
            effective_config: EffectiveConfig {
                tool: self.tool(),
                values,
            },
            diagnostics,
        })
    }

    fn plan(&self, _: PlanRequest<'_>) -> AdapterResult<ChangePlan> {
        Err(read_only())
    }

    fn apply(&self, _: ConfirmedChangePlan) -> AdapterResult<ApplyResult> {
        Err(read_only())
    }

    fn rollback(&self, _: &SnapshotRef) -> AdapterResult<Operation> {
        Err(read_only())
    }

    fn health_check(&self, _: &HealthCheckTarget) -> AdapterResult<HealthCheckResult> {
        Err(read_only())
    }
}

fn command_version(command: &str) -> Option<String> {
    Command::new(command)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|output| output.lines().next().map(str::to_owned))
}

fn mapping_value<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a Value> {
    mapping.get(Value::String(key.into()))
}

fn hosted_url(definition: &Value) -> Result<Option<String>, &'static str> {
    let Some(definition) = definition.as_mapping() else {
        return Ok(None);
    };
    let Some(hosted) = mapping_value(definition, "hosted") else {
        return Ok(None);
    };
    if let Some(url) = hosted.as_str() {
        return Ok(Some(url.into()));
    }
    let Some(hosted) = hosted.as_mapping() else {
        return Err("的 hosted 声明必须是 URL 或映射。");
    };
    let Some(url) = mapping_value(hosted, "url").and_then(Value::as_str) else {
        return Err("的 hosted 映射缺少字符串 url。");
    };
    Ok(Some(url.into()))
}

fn environment_value<'a>(
    environment: &'a BTreeMap<String, String>,
    expected_name: &str,
) -> Option<(&'a str, &'a str)> {
    environment
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(expected_name))
        .map(|(name, value)| (name.as_str(), value.as_str()))
}

fn add_value(
    values: &mut BTreeMap<String, EffectiveValue>,
    key: String,
    value: String,
    scope: ConfigScope,
    location: &str,
    priority: u32,
) {
    let sensitive = value.contains("://***@");
    let entry = values.entry(key).or_insert_with(|| EffectiveValue {
        value: None,
        sources: Vec::new(),
    });
    entry.value = Some(value.clone());
    entry.sources.push(ConfigSource {
        scope,
        location: location.into(),
        priority,
        sensitive,
        value: Some(value),
    });
}

fn redact_credentials(value: &str) -> String {
    let Some(scheme_end) = value.find("://") else {
        return value.into();
    };
    let credential_start = scheme_end + 3;
    let remainder = &value[credential_start..];
    let Some(credential_end) = remainder.find('@') else {
        return value.into();
    };
    let authority_end = remainder
        .find('/')
        .map(|index| credential_start + index)
        .unwrap_or(value.len());
    let credential_end = credential_start + credential_end;
    if credential_end >= authority_end {
        return value.into();
    }

    format!(
        "{}***@{}",
        &value[..credential_start],
        &value[credential_end + 1..]
    )
}

fn read_only() -> AdapterError {
    AdapterError {
        code: AdapterErrorCode::Unsupported,
        message: "Flutter/Pub 目前仅支持只读扫描。".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn reads_default_hosted_url_and_redacts_environment_credentials() {
        let adapter = FlutterPubAdapter::with_environment(BTreeMap::from([
            (
                "PUB_HOSTED_URL".into(),
                "https://packages.example.com/".into(),
            ),
            (
                "https_proxy".into(),
                "https://person:secret@proxy.example:8443".into(),
            ),
        ]));

        let result = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("environment should be readable");

        assert_eq!(
            result.effective_config.values["hosted.default"]
                .value
                .as_deref(),
            Some("https://packages.example.com/")
        );
        assert_eq!(
            result.effective_config.values["hosted.default"]
                .sources
                .len(),
            2
        );
        assert_eq!(
            result.effective_config.values["proxy.https"]
                .value
                .as_deref(),
            Some("https://***@proxy.example:8443")
        );
        assert!(result.effective_config.values["proxy.https"].sources[0].sensitive);
    }

    #[test]
    fn reads_project_hosted_dependencies_without_treating_publish_to_as_a_download_source() {
        let root = test_directory("project");
        fs::write(
            root.join("pubspec.yaml"),
            r#"name: fixture
publish_to: https://publish.example.com/
dependencies:
  direct:
    hosted: https://packages.example.com/
    version: ^1.0.0
dev_dependencies:
  mapped:
    hosted:
      name: mapped
      url: https://dev-packages.example.com/
dependency_overrides:
  local: ^1.0.0
"#,
        )
        .expect("fixture pubspec should be written");
        let adapter = FlutterPubAdapter::with_environment(BTreeMap::new());

        let result = adapter
            .read(&ToolContext {
                project_directory: Some(root.display().to_string()),
                include_project_sources: true,
            })
            .expect("project should be readable");

        assert_eq!(
            result.effective_config.values["publish_to"]
                .value
                .as_deref(),
            Some("https://publish.example.com/")
        );
        assert_eq!(
            result.effective_config.values["dependency.dependencies.direct.hosted"]
                .value
                .as_deref(),
            Some("https://packages.example.com/")
        );
        assert_eq!(
            result.effective_config.values["dependency.dev_dependencies.mapped.hosted"]
                .value
                .as_deref(),
            Some("https://dev-packages.example.com/")
        );
        assert!(!result
            .effective_config
            .values
            .contains_key("dependency.dependency_overrides.local.hosted"));

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn reports_invalid_pubspec_without_hiding_environment_configuration() {
        let root = test_directory("invalid");
        fs::write(root.join("pubspec.yaml"), "dependencies: [").expect("fixture should be written");
        let adapter = FlutterPubAdapter::with_environment(BTreeMap::from([(
            "NO_PROXY".into(),
            "localhost,127.0.0.1".into(),
        )]));

        let result = adapter
            .read(&ToolContext {
                project_directory: Some(root.display().to_string()),
                include_project_sources: true,
            })
            .expect("scan should retain environment values");

        assert_eq!(
            result.effective_config.values["proxy.no_proxy"]
                .value
                .as_deref(),
            Some("localhost,127.0.0.1")
        );
        assert_eq!(result.diagnostics.len(), 1);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    fn test_directory(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("mirrorit-pub-{name}-{timestamp}"));
        fs::create_dir_all(&directory).expect("fixture directory should be created");
        directory
    }
}
