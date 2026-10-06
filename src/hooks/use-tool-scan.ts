import { useCallback, useRef, useState } from "react";
import { scanTool } from "@/lib/api";
import { getToolMeta, type ToolId } from "@/lib/tools";
import type { ToolReadResult } from "@/lib/types";

export type ScanStatus = "idle" | "loading" | "error";

export interface ToolScan {
  result: ToolReadResult | null;
  status: ScanStatus;
  error: string;
}

type ScanMap = Record<ToolId, ToolScan>;

const idleScan: ToolScan = { result: null, status: "idle", error: "" };

function initialScans(): ScanMap {
  return {
    npm: { ...idleScan },
    maven: { ...idleScan },
    "flutter-pub": { ...idleScan },
    go: { ...idleScan },
    cargo: { ...idleScan },
    docker: { ...idleScan },
    pnpm: { ...idleScan },
    yarn: { ...idleScan },
    pip: { ...idleScan },
  };
}

export type ToolAction = () => Promise<ToolReadResult | null | void>;

// 每个工具独立记录状态，并阻止同一工具的重复操作。
export function useToolScans(projectDirectory: string) {
  const [scans, setScans] = useState<ScanMap>(initialScans);
  const pending = useRef(new Set<ToolId>());

  const resetProjectScans = useCallback(() => {
    setScans(
      (current) =>
        Object.fromEntries(
          Object.entries(current).map(([tool, state]) => [
            tool,
            getToolMeta(tool as ToolId).acceptsProjectDirectory
              ? { ...idleScan }
              : state,
          ]),
        ) as ScanMap,
    );
  }, []);

  const operate = useCallback(
    async (tool: ToolId, action: ToolAction): Promise<void> => {
      if (pending.current.has(tool)) return;
      pending.current.add(tool);
      setScans((current) => ({
        ...current,
        [tool]: { ...current[tool], status: "loading", error: "" },
      }));
      try {
        const result = await action();
        setScans((current) => ({
          ...current,
          [tool]: {
            result: result ?? current[tool].result,
            status: "idle",
            error: "",
          },
        }));
      } catch (error) {
        setScans((current) => ({
          ...current,
          [tool]: {
            ...current[tool],
            status: "error",
            error: error instanceof Error ? error.message : String(error),
          },
        }));
      } finally {
        pending.current.delete(tool);
      }
    },
    [],
  );

  const scan = useCallback(
    (tool: ToolId) =>
      operate(tool, () => scanTool(getToolMeta(tool), projectDirectory)),
    [operate, projectDirectory],
  );

  return { scans, scan, operate, resetProjectScans };
}
