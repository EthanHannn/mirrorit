import { getCurrentWindow } from "@tauri-apps/api/window";
import { CheckCircle2 } from "lucide-react";
import type { CSSProperties } from "react";
import { WindowControls } from "@/components/layout/window-controls";
import { ThemeToggle } from "@/components/theme-toggle";

const isTauri = "__TAURI_INTERNALS__" in window;

function toggleMaximize() {
  if (isTauri) {
    void getCurrentWindow().toggleMaximize();
  }
}

export function Titlebar() {
  return (
    <header
      className="glass z-10 flex items-stretch justify-between border-b border-hairline pl-4"
      data-tauri-drag-region
      onDoubleClick={toggleMaximize}
    >
      <div className="flex min-w-0 items-center gap-2.5" data-tauri-drag-region>
        <div
          aria-hidden="true"
          className="grid size-7 place-items-center rounded-md bg-primary text-xs font-bold text-primary-foreground shadow-[0_1px_3px_rgb(0_0_0/18%)]"
        >
          M
        </div>
        <div>
          <p className="text-sm leading-4.5 font-semibold">MirrorIt</p>
          <p className="mt-0.5 text-[0.6875rem] leading-3 text-muted-foreground">
            本机配置中心
          </p>
        </div>
      </div>
      <div
        className="flex items-center gap-3"
        style={{ WebkitAppRegion: "no-drag" } as CSSProperties}
      >
        <span className="flex items-center gap-1.5 text-xs text-muted-foreground max-[760px]:hidden">
          <CheckCircle2 aria-hidden="true" className="size-3.5 text-success" />
          本地模式
        </span>
        <ThemeToggle />
        <WindowControls />
      </div>
    </header>
  );
}
