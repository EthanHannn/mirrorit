import { useCallback, useState } from "react";
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

/**
 * Tracks one independent scan/operation state per tool, so activity in one
 * workspace never leaks into another tool's status.
 */
export function useToolScans(projectDirectory: string) {
  const [scans, setScans] = useState<ScanMap>(initialScans);

  const operate = useCallback(
    async (tool: ToolId, action: ToolAction): Promise<void> => {
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
          [tool]: { ...current[tool], status: "error", error: String(error) },
        }));
      }
    },
    [],
  );

  const scan = useCallback(
    (tool: ToolId) =>
      operate(tool, () => scanTool(getToolMeta(tool), projectDirectory)),
    [operate, projectDirectory],
  );

  return { scans, scan, operate };
}
