import { ShieldCheck } from "lucide-react";
import { toolNavigation, type ToolId } from "@/lib/tools";
import { cn } from "@/lib/utils";

interface SidebarProps {
  activeTool: ToolId;
  onSelect: (tool: ToolId) => void;
  ready: Record<ToolId, boolean>;
}

export function Sidebar({ activeTool, onSelect, ready }: SidebarProps) {
  return (
    <aside className="flex min-h-0 min-w-0 flex-col border-hairline bg-sidebar p-1.5 max-[760px]:border-b min-[760px]:row-span-2 min-[760px]:border-r min-[760px]:px-2.5 min-[760px]:pt-4 min-[760px]:pb-3 min-[1100px]:row-span-1">
      <div className="flex items-center justify-between px-2 pb-2 text-[0.6875rem] font-semibold text-muted-foreground max-[760px]:hidden">
        <span>开发工具</span>
        <span className="tabular-nums">{toolNavigation.length}</span>
      </div>
      <nav
        aria-label="工具导航"
        className="flex gap-0.5 overflow-x-auto min-[760px]:grid min-[760px]:overflow-y-auto"
      >
        {toolNavigation.map((tool) => {
          const active = activeTool === tool.id;
          return (
            <button
              aria-current={active ? "page" : undefined}
              className={cn(
                "grid min-h-11 min-w-0 grid-cols-[2rem_minmax(0,1fr)_auto] items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-muted-foreground outline-none transition-colors duration-150 hover:bg-muted/70 hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/40 active:scale-[0.985] max-[760px]:min-h-9 max-[760px]:min-w-max max-[760px]:grid-cols-[1.75rem_auto]",
                active && "bg-sidebar-accent text-sidebar-accent-foreground",
              )}
              key={tool.id}
              onClick={() => onSelect(tool.id)}
              type="button"
            >
              <span
                aria-hidden="true"
                className="tool-glyph grid size-8 place-items-center rounded-md text-[0.6875rem] font-bold max-[760px]:size-7"
                data-tool={tool.id}
              >
                {tool.glyph}
              </span>
              <span className="grid min-w-0">
                <span className="truncate text-[0.8125rem] font-semibold">
                  {tool.label}
                </span>
                <span className="mt-0.5 text-[0.6875rem] text-muted-foreground max-[760px]:hidden">
                  {tool.mode}
                </span>
              </span>
              <span
                aria-label={ready[tool.id] ? "已读取" : "未读取"}
                className={cn(
                  "size-1.5 rounded-full max-[760px]:hidden",
                  ready[tool.id]
                    ? "bg-success shadow-[0_0_0_2px_color-mix(in_srgb,var(--success)_14%,transparent)]"
                    : "bg-border",
                )}
              />
            </button>
          );
        })}
      </nav>
      <div className="mt-auto flex gap-2 border-t border-hairline px-2 pt-3 pb-0.5 text-xs leading-5 text-muted-foreground max-[760px]:hidden">
        <ShieldCheck
          aria-hidden="true"
          className="mt-1 size-3.5 shrink-0 text-success"
        />
        <p>所有写入都先预览，并自动准备恢复点。</p>
      </div>
    </aside>
  );
}
