use crate::adapters::{AdapterResult, ConfigAdapter, PlanRequest};
use crate::domain::*;
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize, Default)]
struct Settings {
    #[serde(default)]
    mirrors: Mirrors,
    #[serde(default)]
    proxies: Proxies,
    #[serde(default)]
    profiles: Profiles,
}
#[derive(Deserialize, Default)]
struct Mirrors {
    #[serde(rename = "mirror", default)]
    items: Vec<Mirror>,
}
#[derive(Deserialize)]
struct Mirror {
    id: String,
    url: String,
    #[serde(rename = "mirrorOf")]
    mirror_of: String,
}
#[derive(Deserialize, Default)]
struct Proxies {
    #[serde(rename = "proxy", default)]
    items: Vec<Proxy>,
}
#[derive(Deserialize)]
struct Proxy {
    id: Option<String>,
    active: Option<bool>,
    protocol: Option<String>,
    host: String,
    port: Option<u16>,
}
#[derive(Deserialize, Default)]
struct Profiles {
    #[serde(rename = "profile", default)]
    items: Vec<MavenProfile>,
}
#[derive(Deserialize)]
struct MavenProfile {
    id: String,
    #[serde(default)]
    repositories: Repositories,
}
#[derive(Deserialize, Default)]
struct Repositories {
    #[serde(rename = "repository", default)]
    items: Vec<Repository>,
}
#[derive(Deserialize)]
struct Repository {
    id: String,
    url: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct MavenSnapshotRecord {
    snapshot: Snapshot,
    original_content: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct MavenAdapter {
    system_settings_path: Option<PathBuf>,
    user_settings_path: Option<PathBuf>,
    snapshot_directory: Option<PathBuf>,
}

impl MavenAdapter {
    pub fn from_system() -> Self {
        let environment = std::env::vars().collect::<BTreeMap<_, _>>();
        let system_settings_path = system_settings_path(&environment).or_else(|| {
            maven_home_from_command().map(|home| home.join("conf").join("settings.xml"))
        });

        Self {
            system_settings_path,
            user_settings_path: user_settings_path(&environment),
            snapshot_directory: None,
        }
    }

    #[cfg(test)]
    fn with_paths(
        system_settings_path: Option<PathBuf>,
        user_settings_path: Option<PathBuf>,
        snapshot_directory: PathBuf,
    ) -> Self {
        Self {
            system_settings_path,
            user_settings_path,
            snapshot_directory: Some(snapshot_directory),
        }
    }

    pub fn plan_mirror_update(
        &self,
        path: &Path,
        mirror_id: &str,
        next_url: &str,
        _current: &ReadResult,
    ) -> AdapterResult<ChangePlan> {
        validate_mirror_url(next_url)?;
        let xml = fs::read_to_string(path).map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法读取 {}：{error}", path.display()),
        })?;
        crate::adapters::maven_xml::replace_mirror_url(&xml, mirror_id, next_url)?;
        let field = format!("mirror.{mirror_id}");
        Ok(ChangePlan {
            id: format!("maven-mirror-{mirror_id}"),
            tool: self.tool(),
            target_checksums: BTreeMap::new(),
            file_checksums: BTreeMap::from([(
                path.display().to_string(),
                format!("{:x}", Sha256::digest(xml.as_bytes())),
            )]),
            changes: vec![PlannedChange {
                file: path.display().to_string(),
                field: field.clone(),
                previous_value: crate::adapters::maven_xml::mirror_url(&xml, mirror_id)?,
                next_value: Some(next_url.into()),
                risk: None,
            }],
        })
    }
}
impl ConfigAdapter for MavenAdapter {
    fn tool(&self) -> ToolId {
        ToolId::Maven
    }
    fn detect(&self, _: &DetectionContext) -> AdapterResult<ToolDetection> {
        let version = Command::new("mvn")
            .arg("--version")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|output| output.lines().next().map(str::to_owned));

        Ok(ToolDetection {
            tool: self.tool(),
            installed: version.is_some(),
            version,
            capabilities: vec![
                ToolCapability::Read,
                ToolCapability::Plan,
                ToolCapability::Apply,
                ToolCapability::Rollback,
            ],
        })
    }
    fn read(&self, _: &ToolContext) -> AdapterResult<ReadResult> {
        let mut values = BTreeMap::new();
        let mut diagnostics = Vec::new();
        for (scope, priority, path) in self.settings_paths() {
            match fs::read_to_string(&path) {
                Ok(xml) => match from_str::<Settings>(&xml) {
                    Ok(settings) => {
                        for mirror in settings.mirrors.items {
                            let value = format!("{} (mirrorOf: {})", mirror.url, mirror.mirror_of);
                            let source = ConfigSource {
                                scope,
                                location: path.clone(),
                                priority,
                                sensitive: false,
                                value: Some(value.clone()),
                            };
                            let entry = values
                                .entry(format!("mirror.{}", mirror.id))
                                .or_insert_with(|| EffectiveValue {
                                    value: None,
                                    sources: Vec::new(),
                                });
                            entry.value = Some(value);
                            entry.sources.push(source);
                        }
                        for (index, proxy) in settings.proxies.items.into_iter().enumerate() {
                            if proxy.active.unwrap_or(false) {
                                let id = proxy.id.unwrap_or_else(|| format!("proxy-{}", index + 1));
                                let protocol = proxy.protocol.unwrap_or_else(|| "http".into());
                                let port = proxy
                                    .port
                                    .map(|port| format!(":{port}"))
                                    .unwrap_or_default();
                                add_value(
                                    &mut values,
                                    format!("proxy.{id}"),
                                    format!("{protocol}://{}{}", proxy.host, port),
                                    scope,
                                    &path,
                                    priority,
                                );
                            }
                        }
                        for profile in settings.profiles.items {
                            for repository in profile.repositories.items {
                                add_value(
                                    &mut values,
                                    format!("profile.{}.repository.{}", profile.id, repository.id),
                                    repository.url,
                                    scope,
                                    &path,
                                    priority,
                                );
                            }
                        }
                    }
                    Err(error) => diagnostics.push(format!("{path} XML 无法解析：{error}")),
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => diagnostics.push(format!("无法读取 {path}：{error}")),
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
        let (field, value) = request
            .profile
            .values
            .iter()
            .next()
            .ok_or_else(unsupported)?;
        let mirror_id = field.strip_prefix("mirror.").ok_or_else(unsupported)?;
        let path = self
            .user_settings_path
            .as_ref()
            .ok_or_else(|| AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "未能定位用户级 Maven settings.xml，无法生成预览。".into(),
            })?;
        self.plan_mirror_update(path, mirror_id, value.as_str(), request.current_config)
    }

    fn apply(&self, plan: ConfirmedChangePlan) -> AdapterResult<ApplyResult> {
        let plan = plan.into_plan();
        if plan.tool != self.tool() {
            return Err(AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "变更计划不属于 Maven。".into(),
            });
        }
        let (file, expected_checksum) = plan
            .file_checksums
            .iter()
            .next()
            .filter(|_| plan.file_checksums.len() == 1)
            .ok_or_else(|| AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "Maven 变更计划必须且只能包含一个目标文件。".into(),
            })?;
        let change = plan
            .changes
            .first()
            .filter(|change| plan.changes.len() == 1 && change.file == *file)
            .ok_or_else(|| AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "Maven 变更计划必须且只能更新一个一致的字段。".into(),
            })?;
        let mirror_id = change
            .field
            .strip_prefix("mirror.")
            .filter(|id| !id.is_empty())
            .ok_or_else(|| AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "Maven 变更计划包含不受支持的字段。".into(),
            })?;
        let next_url = change.next_value.as_deref().ok_or_else(|| AdapterError {
            code: AdapterErrorCode::InvalidInput,
            message: "Maven 镜像 URL 不能为空。".into(),
        })?;
        validate_mirror_url(next_url)?;

        let path = Path::new(file);
        if file_checksum(path)? != *expected_checksum {
            return Err(AdapterError {
                code: AdapterErrorCode::ExternalModification,
                message: "Maven settings.xml 在预览后发生变化，请重新预览。".into(),
            });
        }
        let original_content = fs::read(path).map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法读取 {}：{error}", path.display()),
        })?;
        let xml = std::str::from_utf8(&original_content).map_err(|_| AdapterError {
            code: AdapterErrorCode::ParseFailure,
            message: "Maven settings.xml 不是 UTF-8 文本，已拒绝写入。".into(),
        })?;
        let next_content =
            crate::adapters::maven_xml::replace_mirror_url(xml, mirror_id, next_url)?;
        let snapshot = self.create_snapshot(path, original_content)?;
        write_atomic(path, next_content.as_bytes(), &snapshot.id)?;

        Ok(ApplyResult {
            operation: Operation {
                id: format!("apply-{}", snapshot.id.0),
                kind: OperationKind::Apply,
                tool: self.tool(),
                outcome: OperationOutcome::Succeeded,
                snapshot: Some(snapshot.id.clone()),
                message: Some("Maven 镜像 URL 已更新，可使用快照回滚。".into()),
            },
            snapshot,
        })
    }

    fn rollback(&self, snapshot: &SnapshotRef) -> AdapterResult<Operation> {
        if !snapshot.0.starts_with("maven-") {
            return Err(AdapterError {
                code: AdapterErrorCode::InvalidInput,
                message: "该快照不属于 Maven。".into(),
            });
        }
        let record_path = self
            .snapshot_directory()?
            .join(format!("{}.json", snapshot.0));
        let content = fs::read(&record_path).map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法读取 Maven 快照 {}：{error}", snapshot.0),
        })?;
        let record: MavenSnapshotRecord =
            serde_json::from_slice(&content).map_err(|_| AdapterError {
                code: AdapterErrorCode::ParseFailure,
                message: "Maven 快照内容无法识别，已拒绝回滚。".into(),
            })?;
        let file = record.snapshot.files.first().ok_or_else(|| AdapterError {
            code: AdapterErrorCode::ParseFailure,
            message: "Maven 快照不包含文件记录。".into(),
        })?;
        write_atomic(Path::new(&file.path), &record.original_content, snapshot)?;

        Ok(Operation {
            id: format!("rollback-{}", snapshot.0),
            kind: OperationKind::Rollback,
            tool: self.tool(),
            outcome: OperationOutcome::Succeeded,
            snapshot: Some(snapshot.clone()),
            message: Some("Maven settings.xml 已从快照恢复。".into()),
        })
    }
    fn health_check(&self, _: &HealthCheckTarget) -> AdapterResult<HealthCheckResult> {
        Err(unsupported())
    }
}
impl MavenAdapter {
    fn settings_paths(&self) -> Vec<(ConfigScope, u32, String)> {
        let mut paths = Vec::new();
        if let Some(path) = &self.system_settings_path {
            paths.push((ConfigScope::System, 100, path.display().to_string()));
        }
        if let Some(path) = &self.user_settings_path {
            paths.push((ConfigScope::User, 200, path.display().to_string()));
        }
        paths
    }

    fn create_snapshot(&self, path: &Path, original_content: Vec<u8>) -> AdapterResult<Snapshot> {
        let created_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: "系统时间不可用，无法创建 Maven 快照。".into(),
            })?
            .as_millis() as i64;
        let directory = self.snapshot_directory()?;
        fs::create_dir_all(&directory).map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法创建 Maven 快照目录：{error}"),
        })?;
        let snapshot_id = next_snapshot_id(&directory, created_at_ms);
        let snapshot = Snapshot {
            id: SnapshotRef(snapshot_id),
            created_at_ms,
            files: vec![SnapshotFile {
                path: path.display().to_string(),
                checksum: format!("{:x}", Sha256::digest(&original_content)),
                permissions: fs::metadata(path)
                    .ok()
                    .map(|metadata| format!("readonly={}", metadata.permissions().readonly())),
            }],
        };
        let record = MavenSnapshotRecord {
            snapshot: snapshot.clone(),
            original_content,
        };
        let content = serde_json::to_vec(&record).map_err(|_| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: "无法序列化 Maven 快照。".into(),
        })?;
        fs::write(directory.join(format!("{}.json", snapshot.id.0)), content).map_err(|error| {
            AdapterError {
                code: AdapterErrorCode::IoFailure,
                message: format!("无法保存 Maven 快照：{error}"),
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

fn next_snapshot_id(directory: &Path, created_at_ms: i64) -> String {
    let base = format!("maven-{created_at_ms}");
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

fn system_settings_path(environment: &BTreeMap<String, String>) -> Option<PathBuf> {
    environment
        .get("M2_HOME")
        .or_else(|| environment.get("MAVEN_HOME"))
        .map(|home| Path::new(home).join("conf/settings.xml"))
}

fn maven_home_from_command() -> Option<PathBuf> {
    let output = Command::new("mvn").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Maven home: "))
        .map(|home| PathBuf::from(home.trim()))
}

fn user_settings_path(environment: &BTreeMap<String, String>) -> Option<PathBuf> {
    environment
        .get("USERPROFILE")
        .or_else(|| environment.get("HOME"))
        .map(|home| Path::new(home).join(".m2/settings.xml"))
}

fn file_checksum(path: &Path) -> AdapterResult<String> {
    fs::read(path)
        .map(|content| format!("{:x}", Sha256::digest(content)))
        .map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法读取 {}：{error}", path.display()),
        })
}

fn validate_mirror_url(value: &str) -> AdapterResult<()> {
    let value = value.trim();
    if !value.starts_with("https://")
        || value[8..].contains('@')
        || value.contains(char::is_whitespace)
    {
        return Err(AdapterError {
            code: AdapterErrorCode::InvalidInput,
            message: "Maven 镜像 URL 必须是未含凭据的 HTTPS 地址。".into(),
        });
    }

    Ok(())
}

fn write_atomic(path: &Path, content: &[u8], snapshot: &SnapshotRef) -> AdapterResult<()> {
    let parent = path.parent().ok_or_else(|| AdapterError {
        code: AdapterErrorCode::IoFailure,
        message: "Maven settings.xml 没有父目录。".into(),
    })?;
    let temporary_path = parent.join(format!(".mirrorit-{}.tmp", snapshot.0));
    fs::write(&temporary_path, content).map_err(|error| AdapterError {
        code: AdapterErrorCode::IoFailure,
        message: format!("无法写入临时 Maven 配置：{error}"),
    })?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法替换 Maven settings.xml：{error}"),
        })?;
    }
    fs::rename(&temporary_path, path).map_err(|error| AdapterError {
        code: AdapterErrorCode::IoFailure,
        message: format!("无法完成 Maven 配置替换：{error}"),
    })
}

fn unsupported() -> AdapterError {
    AdapterError {
        code: AdapterErrorCode::Unsupported,
        message: "Maven 镜像健康检查将在后续阶段提供。".into(),
    }
}
fn add_value(
    values: &mut BTreeMap<String, EffectiveValue>,
    key: String,
    value: String,
    scope: ConfigScope,
    path: &str,
    priority: u32,
) {
    let entry = values.entry(key).or_insert_with(|| EffectiveValue {
        value: None,
        sources: Vec::new(),
    });
    entry.value = Some(value.clone());
    entry.sources.push(ConfigSource {
        scope,
        location: path.into(),
        priority,
        sensitive: false,
        value: Some(value),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_mirrors_proxies_and_profile_repositories() {
        let settings: Settings = from_str(r#"<settings><mirrors><mirror><id>central</id><url>https://mirror.example/</url><mirrorOf>*</mirrorOf></mirror></mirrors><proxies><proxy><id>corp</id><active>true</active><host>proxy.example</host><port>8080</port></proxy></proxies><profiles><profile><id>company</id><repositories><repository><id>internal</id><url>https://repo.example/</url></repository></repositories></profile></profiles></settings>"#).expect("fixture should parse");
        assert_eq!(settings.mirrors.items[0].mirror_of, "*");
        assert!(settings.proxies.items[0]
            .active
            .expect("proxy should be active"));
        assert_eq!(
            settings.profiles.items[0].repositories.items[0].url,
            "https://repo.example/"
        );
    }

    #[test]
    fn applies_a_mirror_preview_with_snapshot_and_restores_original_xml() {
        let root = test_directory("apply-rollback");
        let settings_path = root.join("settings.xml");
        let original = r#"<?xml version="1.0"?><settings><!-- keep --><mirrors><mirror><id>central</id><url>https://previous.example/</url><mirrorOf>*</mirrorOf></mirror><mirror><id>other</id><url>https://other.example/</url></mirror></mirrors><profiles><profile><id>keep</id></profile></profiles></settings>"#;
        fs::write(&settings_path, original).expect("fixture settings should be written");
        let adapter =
            MavenAdapter::with_paths(None, Some(settings_path.clone()), root.join("snapshots"));
        let current = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("fixture settings should be readable");
        let plan = adapter
            .plan_mirror_update(&settings_path, "central", "https://next.example/", &current)
            .expect("preview should succeed");

        assert_eq!(
            plan.changes[0].previous_value.as_deref(),
            Some("https://previous.example/")
        );
        let applied = adapter
            .apply(plan.confirm(1_722_000_000_000))
            .expect("confirmed preview should apply");
        let updated =
            fs::read_to_string(&settings_path).expect("updated settings should be readable");
        assert!(updated.contains("https://next.example/"));
        assert!(updated.contains("https://other.example/"));
        assert!(updated.contains("<!-- keep -->"));

        adapter
            .rollback(&applied.snapshot.id)
            .expect("snapshot should restore");
        assert_eq!(
            fs::read_to_string(&settings_path).expect("restored settings should be readable"),
            original
        );

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn rejects_maven_preview_when_settings_change_externally() {
        let root = test_directory("external-change");
        let settings_path = root.join("settings.xml");
        fs::write(
            &settings_path,
            r#"<settings><mirrors><mirror><id>central</id><url>https://previous.example/</url></mirror></mirrors></settings>"#,
        )
        .expect("fixture settings should be written");
        let adapter =
            MavenAdapter::with_paths(None, Some(settings_path.clone()), root.join("snapshots"));
        let current = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: false,
            })
            .expect("fixture settings should be readable");
        let plan = adapter
            .plan_mirror_update(&settings_path, "central", "https://next.example/", &current)
            .expect("preview should succeed");
        fs::write(
            &settings_path,
            r#"<settings><mirrors><mirror><id>central</id><url>https://external.example/</url></mirror></mirrors></settings>"#,
        )
        .expect("external change should be written");

        let error = adapter
            .apply(plan.confirm(1_722_000_000_000))
            .expect_err("changed settings must reject the preview");
        assert_eq!(error.code, AdapterErrorCode::ExternalModification);

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    #[test]
    fn creates_a_distinct_snapshot_id_when_a_timestamp_is_reused() {
        let root = test_directory("snapshot-id");
        fs::write(root.join("maven-100.json"), b"fixture")
            .expect("fixture snapshot should be written");

        assert_eq!(next_snapshot_id(&root, 100), "maven-100-1");

        fs::remove_dir_all(root).expect("fixture directory should be removed");
    }

    fn test_directory(name: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be available")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("mirrorit-maven-{name}-{timestamp}"));
        fs::create_dir_all(&directory).expect("fixture directory should be created");
        directory
    }
}
