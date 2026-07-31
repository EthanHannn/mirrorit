use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use sha2::{Digest, Sha256};

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
#[cfg(windows)]
use winreg::RegKey;

use crate::adapters::{AdapterResult, ConfigAdapter, PlanRequest};
use crate::domain::{
    AdapterError, AdapterErrorCode, ApplyResult, ChangePlan, ConfigScope, ConfigSource,
    ConfirmedChangePlan, DetectionContext, EffectiveConfig, EffectiveValue, HealthCheckResult,
    HealthCheckTarget, Operation, OperationKind, OperationOutcome, PlannedChange, ReadResult,
    Snapshot, SnapshotFile, SnapshotRef, ToolCapability, ToolContext, ToolDetection, ToolId,
};

const DEFAULT_HOSTED_URL: &str = "https://pub.dev";
const SYSTEM_PRIORITY: u32 = 100;
const PROJECT_PRIORITY: u32 = 200;
const ENVIRONMENT_PRIORITY: u32 = 300;
const USER_HOSTED_TARGET: &str = "registry://HKCU/Environment/PUB_HOSTED_URL";

pub struct FlutterPubAdapter {
    environment: BTreeMap<String, String>,
    hosted_store: Arc<dyn HostedStore>,
    snapshot_directory: Option<PathBuf>,
}

impl FlutterPubAdapter {
    pub fn from_system() -> Self {
        Self {
            environment: std::env::vars().collect(),
            hosted_store: system_hosted_store(),
            snapshot_directory: None,
        }
    }

    #[cfg(test)]
    fn with_environment(environment: BTreeMap<String, String>) -> Self {
        Self {
            environment,
            hosted_store: Arc::new(MemoryHostedStore::default()),
            snapshot_directory: None,
        }
    }

    #[cfg(test)]
    fn with_hosted_store(
        environment: BTreeMap<String, String>,
        initial_value: Option<String>,
        snapshot_directory: PathBuf,
    ) -> Self {
        Self {
            environment,
            hosted_store: Arc::new(MemoryHostedStore::new(initial_value)),
            snapshot_directory: Some(snapshot_directory),
        }
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

    pub fn plan_user_hosted_update(&self, next_value: Option<&str>) -> AdapterResult<ChangePlan> {
        if let Some(value) = next_value {
            validate_hosted_url(value)?;
        }
        let current = self.hosted_store.read()?;
        Ok(ChangePlan {
            id: "flutter-pub-user-hosted".into(),
            tool: self.tool(),
            target_checksums: BTreeMap::from([(
                USER_HOSTED_TARGET.into(),
                hosted_checksum(current.as_ref()),
            )]),
            file_checksums: BTreeMap::new(),
            changes: vec![PlannedChange {
                file: USER_HOSTED_TARGET.into(),
                field: "hosted.default".into(),
                previous_value: current.as_ref().map(|value| value.value.clone()),
                next_value: next_value.map(str::to_owned),
                risk: Some(
                    "重新启动 Flutter、终端和 MirrorIt 后，新的用户环境变量才会生效。".into(),
                ),
            }],
        })
    }

    fn create_snapshot(&self, original_value: Option<HostedValue>) -> AdapterResult<Snapshot> {
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: "系统时间不可用，无法创建 Flutter/Pub 快照。".into(),
            })?
            .as_millis() as i64;
        let directory = self.snapshot_directory()?;
        fs::create_dir_all(&directory).map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法创建 Flutter/Pub 快照目录：{error}"),
        })?;
        let id = next_snapshot_id(&directory, created_at_ms);
        let snapshot = Snapshot {
            id: SnapshotRef(id),
            created_at_ms,
            files: vec![SnapshotFile {
                path: USER_HOSTED_TARGET.into(),
                checksum: hosted_checksum(original_value.as_ref()),
                permissions: None,
            }],
        };
        let record = FlutterPubSnapshotRecord {
            snapshot: snapshot.clone(),
            original_value,
        };
        let content = serde_json::to_vec(&record).map_err(|_| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: "无法序列化 Flutter/Pub 快照。".into(),
        })?;
        fs::write(directory.join(format!("{}.json", snapshot.id.0)), content).map_err(|error| {
            AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: format!("无法保存 Flutter/Pub 快照：{error}"),
            }
        })?;

        Ok(snapshot)
    }

    fn snapshot_directory(&self) -> AdapterResult<PathBuf> {
        if let Some(directory) = &self.snapshot_directory {
            return Ok(directory.clone());
        }
        let base = std::env::var("LOCALAPPDATA")
            .or_else(|_| std::env::var("APPDATA"))
            .map_err(|_| AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: "未能定位本地应用数据目录。".into(),
            })?;
        Ok(Path::new(&base).join("MirrorIt").join("snapshots"))
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

        match self.hosted_store.read() {
            Ok(Some(value)) => add_value(
                &mut values,
                "hosted.default".into(),
                redact_credentials(&value.value),
                ConfigScope::Environment,
                "HKCU\\Environment\\PUB_HOSTED_URL",
                ENVIRONMENT_PRIORITY,
            ),
            Ok(None) | Err(_) => {
                if let Some((name, value)) = environment_value(&self.environment, "PUB_HOSTED_URL")
                {
                    add_value(
                        &mut values,
                        "hosted.default".into(),
                        redact_credentials(value),
                        ConfigScope::Environment,
                        name,
                        ENVIRONMENT_PRIORITY,
                    );
                }
            }
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

    fn plan(&self, request: PlanRequest<'_>) -> AdapterResult<ChangePlan> {
        let value = request
            .profile
            .values
            .get("hosted.default")
            .ok_or_else(read_only)?;
        let next_value = (!value.as_str().is_empty()).then_some(value.as_str());
        self.plan_user_hosted_update(next_value)
    }

    fn apply(&self, plan: ConfirmedChangePlan) -> AdapterResult<ApplyResult> {
        let plan = plan.into_plan();
        if plan.tool != self.tool()
            || !plan.file_checksums.is_empty()
            || plan.target_checksums.len() != 1
            || plan.changes.len() != 1
        {
            return Err(AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "Flutter/Pub 变更计划不符合用户 hosted 源范围。".into(),
            });
        }
        let expected_checksum = plan
            .target_checksums
            .get(USER_HOSTED_TARGET)
            .ok_or_else(|| AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "Flutter/Pub 变更计划缺少用户环境变量校验目标。".into(),
            })?;
        let change = &plan.changes[0];
        if change.file != USER_HOSTED_TARGET || change.field != "hosted.default" {
            return Err(AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "Flutter/Pub 变更计划包含不受支持的字段。".into(),
            });
        }
        if let Some(value) = change.next_value.as_deref() {
            validate_hosted_url(value)?;
        }
        let current = self.hosted_store.read()?;
        if hosted_checksum(current.as_ref()) != *expected_checksum {
            return Err(AdapterError {
                code: AdapterErrorCode::ExternalModification,
                message: "PUB_HOSTED_URL 在预览后发生变化，请重新预览。".into(),
            });
        }
        let snapshot = self.create_snapshot(current)?;
        self.hosted_store.write(change.next_value.as_deref())?;

        Ok(ApplyResult {
            operation: Operation {
                id: format!("apply-{}", snapshot.id.0),
                kind: OperationKind::Apply,
                tool: self.tool(),
                outcome: OperationOutcome::Succeeded,
                snapshot: Some(snapshot.id.clone()),
                message: Some(
                    "用户级 PUB_HOSTED_URL 已更新；重新启动 Flutter、终端和 MirrorIt 后生效。"
                        .into(),
                ),
            },
            snapshot,
        })
    }

    fn rollback(&self, snapshot: &SnapshotRef) -> AdapterResult<Operation> {
        if !snapshot.0.starts_with("flutter-pub-") {
            return Err(AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "该快照不属于 Flutter/Pub。".into(),
            });
        }
        let content = fs::read(
            self.snapshot_directory()?
                .join(format!("{}.json", snapshot.0)),
        )
        .map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法读取 Flutter/Pub 快照 {}：{error}", snapshot.0),
        })?;
        let record: FlutterPubSnapshotRecord =
            serde_json::from_slice(&content).map_err(|_| AdapterError {
                code: AdapterErrorCode::ParseFailure,
                message: "Flutter/Pub 快照内容无法识别，已拒绝回滚。".into(),
            })?;
        self.hosted_store.write(
            record
                .original_value
                .as_ref()
                .map(|value| value.value.as_str()),
        )?;

        Ok(Operation {
            id: format!("rollback-{}", snapshot.0),
            kind: OperationKind::Rollback,
            tool: self.tool(),
            outcome: OperationOutcome::Succeeded,
            snapshot: Some(snapshot.clone()),
            message: Some(
                "PUB_HOSTED_URL 已从快照恢复；重新启动 Flutter、终端和 MirrorIt 后生效。".into(),
            ),
        })
    }

    fn health_check(&self, _: &HealthCheckTarget) -> AdapterResult<HealthCheckResult> {
        Err(read_only())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HostedValue {
    value: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FlutterPubSnapshotRecord {
    snapshot: Snapshot,
    original_value: Option<HostedValue>,
}

trait HostedStore: Send + Sync {
    fn read(&self) -> AdapterResult<Option<HostedValue>>;
    fn write(&self, value: Option<&str>) -> AdapterResult<()>;
}

#[cfg(windows)]
struct RegistryHostedStore;

#[cfg(windows)]
impl HostedStore for RegistryHostedStore {
    fn read(&self) -> AdapterResult<Option<HostedValue>> {
        let environment = environment_key(KEY_READ)?;
        match environment.get_value::<String, _>("PUB_HOSTED_URL") {
            Ok(value) => Ok(Some(HostedValue { value })),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: format!("无法读取用户环境变量 PUB_HOSTED_URL：{error}"),
            }),
        }
    }

    fn write(&self, value: Option<&str>) -> AdapterResult<()> {
        let environment = environment_key(KEY_WRITE)?;
        match value {
            Some(value) => environment.set_value("PUB_HOSTED_URL", &value),
            None => match environment.delete_value("PUB_HOSTED_URL") {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            },
        }
        .map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法更新用户环境变量 PUB_HOSTED_URL：{error}"),
        })
    }
}

#[cfg(windows)]
fn environment_key(access: u32) -> AdapterResult<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", access)
        .map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法打开用户环境变量存储：{error}"),
        })
}

#[cfg(not(windows))]
struct UnsupportedHostedStore;

#[cfg(not(windows))]
impl HostedStore for UnsupportedHostedStore {
    fn read(&self) -> AdapterResult<Option<HostedValue>> {
        Err(platform_unsupported())
    }

    fn write(&self, _: Option<&str>) -> AdapterResult<()> {
        Err(platform_unsupported())
    }
}

fn system_hosted_store() -> Arc<dyn HostedStore> {
    #[cfg(windows)]
    {
        Arc::new(RegistryHostedStore)
    }
    #[cfg(not(windows))]
    {
        Arc::new(UnsupportedHostedStore)
    }
}

#[cfg(not(windows))]
fn platform_unsupported() -> AdapterError {
    AdapterError {
        code: AdapterErrorCode::Unsupported,
        message: "Flutter/Pub 用户 hosted 源写入目前仅支持 Windows。".into(),
    }
}

#[cfg(test)]
#[derive(Default)]
struct MemoryHostedStore(Mutex<Option<HostedValue>>);

#[cfg(test)]
impl MemoryHostedStore {
    fn new(value: Option<String>) -> Self {
        Self(Mutex::new(value.map(|value| HostedValue { value })))
    }
}

#[cfg(test)]
impl HostedStore for MemoryHostedStore {
    fn read(&self) -> AdapterResult<Option<HostedValue>> {
        self.0
            .lock()
            .map(|value| value.clone())
            .map_err(|_| AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: "测试存储不可用。".into(),
            })
    }

    fn write(&self, value: Option<&str>) -> AdapterResult<()> {
        let mut current = self.0.lock().map_err(|_| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: "测试存储不可用。".into(),
        })?;
        *current = value.map(|value| HostedValue {
            value: value.into(),
        });
        Ok(())
    }
}

fn hosted_checksum(value: Option<&HostedValue>) -> String {
    let value = value
        .map(|value| value.value.as_str())
        .unwrap_or("<missing>");
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn next_snapshot_id(directory: &Path, created_at_ms: i64) -> String {
    let base = format!("flutter-pub-{created_at_ms}");
    let mut suffix = 0;
    loop {
        let id = if suffix == 0 {
            base.clone()
        } else {
            format!("{base}-{suffix}")
        };
        if !directory.join(format!("{id}.json")).exists() {
            return id;
        }
        suffix += 1;
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

fn validate_hosted_url(value: &str) -> AdapterResult<()> {
    let value = value.trim();
    if !value.starts_with("https://")
        || value[8..].contains('@')
        || value.contains(char::is_whitespace)
    {
        return Err(AdapterError {
            code: AdapterErrorCode::InvalidInput,
            message: "Flutter/Pub hosted 源必须是未含凭据的 HTTPS 地址。".into(),
        });
    }
    Ok(())
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

    #[test]
    fn applies_a_hosted_preview_with_snapshot_and_restores_it() {
        let root = test_directory("apply-rollback");
        let adapter = FlutterPubAdapter::with_hosted_store(
            BTreeMap::new(),
            Some("https://previous.example/".into()),
            root.join("snapshots"),
        );
        let plan = adapter
            .plan_user_hosted_update(Some("https://next.example/"))
            .expect("preview should succeed");
        let applied = adapter
            .apply(plan.confirm(1_722_000_000_000))
            .expect("confirmed preview should apply");

        assert_eq!(
            adapter
                .hosted_store
                .read()
                .expect("fixture store should be readable")
                .as_ref()
                .map(|value| value.value.as_str()),
            Some("https://next.example/")
        );
        adapter
            .rollback(&applied.snapshot.id)
            .expect("snapshot should restore");
        assert_eq!(
            adapter
                .hosted_store
                .read()
                .expect("fixture store should be readable")
                .as_ref()
                .map(|value| value.value.as_str()),
            Some("https://previous.example/")
        );

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn rejects_a_hosted_preview_after_an_external_change() {
        let root = test_directory("external-change");
        let adapter = FlutterPubAdapter::with_hosted_store(
            BTreeMap::new(),
            Some("https://previous.example/".into()),
            root.join("snapshots"),
        );
        let plan = adapter
            .plan_user_hosted_update(Some("https://next.example/"))
            .expect("preview should succeed");
        adapter
            .hosted_store
            .write(Some("https://external.example/"))
            .expect("fixture store should be writable");

        let error = adapter
            .apply(plan.confirm(1_722_000_000_000))
            .expect_err("external change must reject the preview");
        assert_eq!(error.code, AdapterErrorCode::ExternalModification);

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
