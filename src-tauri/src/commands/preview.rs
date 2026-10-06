use sha2::{Digest, Sha256};

use crate::domain::ChangePlan;

pub fn identify_plan(mut plan: ChangePlan) -> Result<ChangePlan, String> {
    // 标识绑定完整预览内容，旧确认不能引用另一次预览。
    let content = serde_json::to_vec(&plan).map_err(|_| "无法生成预览标识。".to_owned())?;
    plan.id = format!("{}-{:x}", plan.id, Sha256::digest(content));
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{PlannedChange, ToolId};
    use std::collections::BTreeMap;

    fn plan() -> ChangePlan {
        ChangePlan {
            id: "npm-custom-user".into(),
            tool: ToolId::Npm,
            target_checksums: BTreeMap::new(),
            file_checksums: BTreeMap::from([("fixture/.npmrc".into(), "missing".into())]),
            changes: vec![PlannedChange {
                file: "fixture/.npmrc".into(),
                field: "registry".into(),
                previous_value: None,
                next_value: Some("https://registry.npmjs.org/".into()),
                risk: None,
            }],
        }
    }

    #[test]
    fn identical_previews_have_the_same_identity() {
        assert_eq!(
            identify_plan(plan()).unwrap().id,
            identify_plan(plan()).unwrap().id
        );
    }

    #[test]
    fn changed_value_or_target_cannot_replace_an_existing_preview() {
        let original = identify_plan(plan()).unwrap().id;
        let mut changed = plan();
        changed.changes[0].next_value = Some("https://registry.npmmirror.com/".into());
        assert_ne!(original, identify_plan(changed).unwrap().id);
        let mut moved = plan();
        moved.file_checksums = BTreeMap::from([("other/.npmrc".into(), "missing".into())]);
        assert_ne!(original, identify_plan(moved).unwrap().id);
        let mut modified = plan();
        modified
            .file_checksums
            .insert("fixture/.npmrc".into(), "modified".into());
        assert_ne!(original, identify_plan(modified).unwrap().id);
    }
}
