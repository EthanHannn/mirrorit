import { CircleAlert } from "lucide-react";
import { ThemeProvider } from "@/components/theme-provider";
import { ThemeToggle } from "@/components/theme-toggle";

function App() {
  return (
    <ThemeProvider>
      <div className="min-h-svh bg-background text-foreground transition-colors duration-200 motion-reduce:transition-none">
        <header className="flex h-14 items-center justify-between border-b border-border bg-background/85 px-5 backdrop-blur-sm">
          <div className="flex items-center gap-3">
            <div
              aria-hidden="true"
              className="grid size-7 place-items-center rounded-md bg-primary text-xs font-semibold text-primary-foreground"
            >
              M
            </div>
            <div>
              <p className="text-sm font-semibold leading-4">MirrorIt</p>
              <p className="mt-0.5 text-xs leading-3 text-muted-foreground">
                本地配置工作台
              </p>
            </div>
          </div>
          <ThemeToggle />
        </header>

        <main className="mx-auto w-full max-w-5xl px-5 py-10">
          <section aria-labelledby="overview-heading">
            <p className="text-xs font-medium text-muted-foreground">总览</p>
            <h1 id="overview-heading" className="mt-1 text-xl font-semibold">
              配置状态
            </h1>

            <div className="mt-6 flex min-h-44 items-center gap-3 border-y border-border py-8">
              <CircleAlert
                aria-hidden="true"
                className="size-5 shrink-0 text-warning"
              />
              <div>
                <p className="text-sm font-medium">尚未扫描本机配置</p>
                <p className="mt-1 text-sm text-muted-foreground">
                  工具发现完成后，此处将显示当前环境状态。
                </p>
              </div>
            </div>
          </section>
        </main>
      </div>
    </ThemeProvider>
  );
}

export default App;
