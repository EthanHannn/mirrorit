use crate::adapters::{AdapterResult, ConfigAdapter, PlanRequest};
use crate::domain::*;
use quick_xml::de::from_str;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

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
pub struct MavenAdapter;
impl MavenAdapter {
    pub fn from_system() -> Self {
        Self
    }

    pub fn plan_mirror_update(
        &self,
        path: &Path,
        mirror_id: &str,
        next_url: &str,
        current: &ReadResult,
    ) -> AdapterResult<ChangePlan> {
        let xml = fs::read_to_string(path).map_err(|error| AdapterError {
            code: AdapterErrorCode::IoFailure,
            message: format!("无法读取 {}：{error}", path.display()),
        })?;
        crate::adapters::maven_xml::replace_mirror_url(&xml, mirror_id, next_url)?;
        let field = format!("mirror.{mirror_id}");
        Ok(ChangePlan {
            id: format!("maven-mirror-{mirror_id}"),
            tool: self.tool(),
            file_checksums: BTreeMap::from([(
                path.display().to_string(),
                format!("{:x}", Sha256::digest(xml.as_bytes())),
            )]),
            changes: vec![PlannedChange {
                file: path.display().to_string(),
                field: field.clone(),
                previous_value: current
                    .effective_config
                    .values
                    .get(&field)
                    .and_then(|value| value.value.clone()),
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
        Ok(ToolDetection {
            tool: self.tool(),
            installed: false,
            version: None,
            capabilities: vec![ToolCapability::Read],
        })
    }
    fn read(&self, _: &ToolContext) -> AdapterResult<ReadResult> {
        let mut values = BTreeMap::new();
        let mut diagnostics = Vec::new();
        for (scope, priority, path) in settings_paths() {
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
        let path = settings_paths()
            .into_iter()
            .find(|(scope, _, _)| *scope == ConfigScope::User)
            .map(|(_, _, path)| path)
            .ok_or_else(unsupported)?;
        self.plan_mirror_update(
            Path::new(&path),
            mirror_id,
            value.as_str(),
            request.current_config,
        )
    }
    fn apply(&self, _: ConfirmedChangePlan) -> AdapterResult<ApplyResult> {
        Err(unsupported())
    }
    fn rollback(&self, _: &SnapshotRef) -> AdapterResult<Operation> {
        Err(unsupported())
    }
    fn health_check(&self, _: &HealthCheckTarget) -> AdapterResult<HealthCheckResult> {
        Err(unsupported())
    }
}
fn settings_paths() -> Vec<(ConfigScope, u32, String)> {
    let mut paths = Vec::new();
    if let Ok(home) = std::env::var("M2_HOME").or_else(|_| std::env::var("MAVEN_HOME")) {
        paths.push((
            ConfigScope::System,
            100,
            Path::new(&home)
                .join("conf/settings.xml")
                .display()
                .to_string(),
        ));
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        paths.push((
            ConfigScope::User,
            200,
            Path::new(&home)
                .join(".m2/settings.xml")
                .display()
                .to_string(),
        ));
    }
    paths
}
fn unsupported() -> AdapterError {
    AdapterError {
        code: AdapterErrorCode::Unsupported,
        message: "Maven 配置写入将在后续阶段提供。".into(),
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
}
