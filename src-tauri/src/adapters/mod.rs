use crate::domain::{
    AdapterError, ApplyResult, ChangePlan, ConfirmedChangePlan, DetectionContext,
    HealthCheckResult, HealthCheckTarget, Operation, Profile, ReadResult, SnapshotRef, ToolContext,
    ToolDetection, ToolId,
};

pub type AdapterResult<T> = Result<T, AdapterError>;

pub mod cargo;
pub mod docker;
pub mod flutter_pub;
pub mod go;
pub mod maven;
pub mod maven_xml;
pub mod npm;

pub struct PlanRequest<'a> {
    pub profile: &'a Profile,
    pub current_config: &'a ReadResult,
}

pub trait ConfigAdapter {
    fn tool(&self) -> ToolId;

    fn detect(&self, context: &DetectionContext) -> AdapterResult<ToolDetection>;

    fn read(&self, context: &ToolContext) -> AdapterResult<ReadResult>;

    fn plan(&self, request: PlanRequest<'_>) -> AdapterResult<ChangePlan>;

    fn apply(&self, plan: ConfirmedChangePlan) -> AdapterResult<ApplyResult>;

    fn rollback(&self, snapshot: &SnapshotRef) -> AdapterResult<Operation>;

    fn health_check(&self, target: &HealthCheckTarget) -> AdapterResult<HealthCheckResult>;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::domain::{
        ConfigScope, ConfigSource, EffectiveConfig, EffectiveValue, HealthCheckStatus,
        NonSensitiveValue, OperationKind, OperationOutcome, PlannedChange, Snapshot, SnapshotFile,
        ToolCapability,
    };

    use super::*;

    struct FixtureAdapter;

    impl ConfigAdapter for FixtureAdapter {
        fn tool(&self) -> ToolId {
            ToolId::Npm
        }

        fn detect(&self, _context: &DetectionContext) -> AdapterResult<ToolDetection> {
            Ok(ToolDetection {
                tool: self.tool(),
                installed: true,
                version: Some("10.0.0".into()),
                capabilities: vec![
                    ToolCapability::Read,
                    ToolCapability::Plan,
                    ToolCapability::Apply,
                    ToolCapability::Rollback,
                    ToolCapability::HealthCheck,
                ],
            })
        }

        fn read(&self, _context: &ToolContext) -> AdapterResult<ReadResult> {
            Ok(ReadResult {
                effective_config: EffectiveConfig {
                    tool: self.tool(),
                    values: BTreeMap::from([(
                        "registry".into(),
                        EffectiveValue {
                            value: Some("https://registry.npmjs.org/".into()),
                            sources: vec![ConfigSource {
                                scope: ConfigScope::User,
                                location: "C:/Users/example/.npmrc".into(),
                                priority: 10,
                                sensitive: false,
                                value: Some("https://registry.npmjs.org/".into()),
                            }],
                        },
                    )]),
                },
                diagnostics: Vec::new(),
            })
        }

        fn plan(&self, request: PlanRequest<'_>) -> AdapterResult<ChangePlan> {
            let current_value = request
                .current_config
                .effective_config
                .values
                .get("registry")
                .and_then(|value| value.value.clone());
            let next_value = request
                .profile
                .values
                .get("registry")
                .map(|value| value.as_str().to_owned());

            Ok(ChangePlan {
                id: "plan-1".into(),
                tool: self.tool(),
                target_checksums: BTreeMap::new(),
                file_checksums: BTreeMap::from([(
                    "C:/Users/example/.npmrc".into(),
                    "fixture-checksum".into(),
                )]),
                changes: vec![PlannedChange {
                    file: "C:/Users/example/.npmrc".into(),
                    field: "registry".into(),
                    previous_value: current_value,
                    next_value,
                    risk: None,
                }],
            })
        }

        fn apply(&self, plan: ConfirmedChangePlan) -> AdapterResult<ApplyResult> {
            let snapshot = Snapshot {
                id: SnapshotRef("snapshot-1".into()),
                created_at_ms: plan.confirmed_at_ms,
                files: vec![SnapshotFile {
                    path: "C:/Users/example/.npmrc".into(),
                    checksum: "fixture-checksum".into(),
                    permissions: None,
                }],
            };

            Ok(ApplyResult {
                operation: Operation {
                    id: format!("apply-{}", plan.plan().id),
                    kind: OperationKind::Apply,
                    tool: self.tool(),
                    outcome: OperationOutcome::Succeeded,
                    snapshot: Some(snapshot.id.clone()),
                    message: None,
                },
                snapshot,
            })
        }

        fn rollback(&self, snapshot: &SnapshotRef) -> AdapterResult<Operation> {
            Ok(Operation {
                id: format!("rollback-{}", snapshot.0),
                kind: OperationKind::Rollback,
                tool: self.tool(),
                outcome: OperationOutcome::Succeeded,
                snapshot: Some(snapshot.clone()),
                message: None,
            })
        }

        fn health_check(&self, target: &HealthCheckTarget) -> AdapterResult<HealthCheckResult> {
            Ok(HealthCheckResult {
                target: target.clone(),
                status: HealthCheckStatus::Healthy,
                elapsed_ms: 12,
                message: None,
            })
        }
    }

    #[test]
    fn adapter_contract_operations_are_individually_testable() {
        let adapter = FixtureAdapter;
        let detection = adapter
            .detect(&DetectionContext {
                environment: BTreeMap::new(),
            })
            .expect("fixture detection should succeed");
        assert!(detection.installed);

        let current_config = adapter
            .read(&ToolContext {
                project_directory: None,
                include_project_sources: true,
            })
            .expect("fixture read should succeed");
        assert_eq!(
            current_config.effective_config.values["registry"]
                .value
                .as_deref(),
            Some("https://registry.npmjs.org/")
        );

        let profile = Profile {
            id: "official-npm".into(),
            name: "Official npm".into(),
            values: BTreeMap::from([(
                "registry".into(),
                NonSensitiveValue::new("https://registry.npmjs.org/"),
            )]),
        };
        let plan = adapter
            .plan(PlanRequest {
                profile: &profile,
                current_config: &current_config,
            })
            .expect("fixture plan should succeed");
        assert_eq!(plan.changes.len(), 1);

        let applied = adapter
            .apply(plan.confirm(1_722_000_000_000))
            .expect("fixture apply should succeed");
        assert_eq!(applied.operation.kind, OperationKind::Apply);

        let rolled_back = adapter
            .rollback(&applied.snapshot.id)
            .expect("fixture rollback should succeed");
        assert_eq!(rolled_back.kind, OperationKind::Rollback);

        let health = adapter
            .health_check(&HealthCheckTarget {
                address: "https://registry.npmjs.org/".into(),
            })
            .expect("fixture health check should succeed");
        assert_eq!(health.status, HealthCheckStatus::Healthy);
    }
}
