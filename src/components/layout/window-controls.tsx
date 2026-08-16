import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";

const isTauri = "__TAURI_INTERNALS__" in window;

const controlClass =
  "grid h-full w-11 shrink-0 place-items-center text-muted-foreground outline-none transition-colors hover:bg-muted hover:text-foreground focus-visible:bg-muted";

/**
 * Frameless-window controls. Rendered only inside the Tauri runtime; plain
 * browser previews (vite dev) keep the native browser chrome instead.
 */
export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri) {
      return;
    }
    const appWindow = getCurrentWindow();
    void appWindow.isMaximized().then(setMaximized);
    const unlisten = appWindow.onResized(() => {
      void appWindow.isMaximized().then(setMaximized);
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  if (!isTauri) {
    return null;
  }

  const appWindow = getCurrentWindow();

  return (
    <div className="-mr-4 flex h-full items-stretch">
      <button
        aria-label="最小化"
        className={controlClass}
        onClick={() => void appWindow.minimize()}
        type="button"
      >
        <Minus aria-hidden="true" className="size-4" />
      </button>
      <button
        aria-label={maximized ? "还原" : "最大化"}
        className={controlClass}
        onClick={() => void appWindow.toggleMaximize()}
        type="button"
      >
        {maximized ? (
          <Copy aria-hidden="true" className="size-3.5" />
        ) : (
          <Square aria-hidden="true" className="size-3.5" />
        )}
      </button>
      <button
        aria-label="关闭"
        className={cn(
          controlClass,
          "hover:bg-destructive hover:text-white focus-visible:bg-destructive focus-visible:text-white",
        )}
        onClick={() => void appWindow.close()}
        type="button"
      >
        <X aria-hidden="true" className="size-4" />
      </button>
    </div>
  );
}
