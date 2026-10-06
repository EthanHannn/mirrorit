import assert from "node:assert/strict";
import { test } from "node:test";
import { applyAndRefresh } from "../src/lib/operations.ts";

test("写入成功但扫描失败时仍保存恢复点并废弃预览", async () => {
  const events: string[] = [];
  await assert.rejects(
    applyAndRefresh(
      async () => ({ snapshot: { id: "npm-snapshot" } }),
      (id) => events.push(id),
      () => events.push("clear"),
      async () => {
        throw new Error("无法读取");
      },
    ),
    /配置已更新，恢复点已保存/,
  );
  assert.deepEqual(events, ["npm-snapshot", "clear"]);
});

test("写入失败时保留已有恢复点且要求重新预览", async () => {
  let snapshot = "previous";
  let cleared = false;
  await assert.rejects(
    applyAndRefresh(
      async () => {
        throw new Error("预览已失效");
      },
      (id) => {
        snapshot = id;
      },
      () => {
        cleared = true;
      },
      async () => {
        throw new Error("不应执行扫描");
      },
    ),
    /预览已失效/,
  );
  assert.equal(snapshot, "previous");
  assert.equal(cleared, true);
});

test("完成写入后返回重新读取的配置", async () => {
  const result = { effective_config: { values: {} }, diagnostics: [] };
  assert.equal(
    await applyAndRefresh(
      async () => ({ snapshot: { id: "snapshot" } }),
      () => {},
      () => {},
      async () => result,
    ),
    result,
  );
});
