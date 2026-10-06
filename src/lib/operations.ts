import type { ApplyResult, ToolReadResult } from "@/lib/types";

export async function applyAndRefresh(
  apply: () => Promise<ApplyResult>,
  rememberSnapshot: (id: string) => void,
  clearPlan: () => void,
  refresh: () => Promise<ToolReadResult>,
): Promise<ToolReadResult> {
  let applied: ApplyResult;
  try {
    applied = await apply();
  } catch (error) {
    clearPlan();
    throw error;
  }
  // 写入结果先保存，后续读取失败也必须保留恢复入口。
  rememberSnapshot(applied.snapshot.id);
  clearPlan();
  try {
    return await refresh();
  } catch {
    throw new Error(
      "配置已更新，恢复点已保存，但重新扫描失败。请重新扫描确认当前配置；需要撤销时可从恢复点还原。",
    );
  }
}
