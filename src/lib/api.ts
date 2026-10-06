import { invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
import type { ToolMeta } from "@/lib/tools";
import type {
  ApplyResult,
  ChangePlan,
  HealthCheckResult,
  NpmProfileImportPreview,
  NpmProfileSelection,
  ProfileExportDocument,
  TargetScope,
  ToolReadResult,
} from "@/lib/types";

function invoke<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri()) {
    return Promise.reject(
      new Error(
        "读取本机配置需要 MirrorIt 桌面客户端。当前为浏览器预览，请在桌面窗口中执行此操作。",
      ),
    );
  }
  return tauriInvoke<T>(command, args);
}

export function scanTool(
  meta: ToolMeta,
  projectDirectory: string,
): Promise<ToolReadResult> {
  return invoke<ToolReadResult>(
    meta.scanCommand,
    meta.acceptsProjectDirectory
      ? { projectDirectory: projectDirectory.trim() || null }
      : undefined,
  );
}

export function previewNpmProfile(request: {
  projectDirectory: string | null;
  targetScope: TargetScope;
  profile: NpmProfileSelection;
}): Promise<ChangePlan> {
  return invoke<ChangePlan>("preview_npm_profile", { request });
}

export function applyNpmPreview(planId: string): Promise<ApplyResult> {
  return invoke<ApplyResult>("apply_npm_preview", { planId });
}

export function rollbackNpmSnapshot(snapshotId: string): Promise<void> {
  return invoke("rollback_npm_snapshot", { snapshotId });
}

export function checkNpmHealth(address: string): Promise<HealthCheckResult> {
  return invoke<HealthCheckResult>("check_npm_health", { address });
}

export function exportNpmProfile(request: {
  profile: NpmProfileSelection;
}): Promise<ProfileExportDocument> {
  return invoke<ProfileExportDocument>("export_npm_profile", { request });
}

export function previewNpmProfileImport(request: {
  content: string;
  currentRegistry: string;
}): Promise<NpmProfileImportPreview> {
  return invoke<NpmProfileImportPreview>("preview_npm_profile_import", {
    request,
  });
}

export function previewMavenMirrorUpdate(request: {
  mirrorId: string;
  url: string;
}): Promise<ChangePlan> {
  return invoke<ChangePlan>("preview_maven_mirror_update", { request });
}

export function applyMavenPreview(planId: string): Promise<ApplyResult> {
  return invoke<ApplyResult>("apply_maven_preview", { planId });
}

export function rollbackMavenSnapshot(snapshotId: string): Promise<void> {
  return invoke("rollback_maven_snapshot", { snapshotId });
}

export function previewFlutterPubHostedUpdate(request: {
  hostedUrl: string | null;
}): Promise<ChangePlan> {
  return invoke<ChangePlan>("preview_flutter_pub_hosted_update", { request });
}

export function applyFlutterPubPreview(planId: string): Promise<ApplyResult> {
  return invoke<ApplyResult>("apply_flutter_pub_preview", { planId });
}

export function rollbackFlutterPubSnapshot(snapshotId: string): Promise<void> {
  return invoke("rollback_flutter_pub_snapshot", { snapshotId });
}
