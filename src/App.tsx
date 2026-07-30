import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  CircleAlert,
  FileDiff,
  FolderSearch,
  LoaderCircle,
  PackageSearch,
  RefreshCw,
  RotateCcw,
  Save,
  ShieldCheck,
  Terminal,
} from "lucide-react";
import { useState } from "react";
import { ThemeProvider } from "@/components/theme-provider";
import { ThemeToggle } from "@/components/theme-toggle";
import { Button } from "@/components/ui/button";

type ConfigScope = "user" | "project" | "environment";

interface ConfigSource {
  scope: ConfigScope;
  location: string;
  priority: number;
  sensitive: boolean;
  value: string | null;
}

interface EffectiveValue {
  value: string | null;
  sources: ConfigSource[];
}

interface NpmReadResult {
  effective_config: {
    values: Record<string, EffectiveValue>;
  };
  diagnostics: string[];
}

interface ChangePlan {
  id: string;
  file_checksums: Record<string, string>;
  changes: PlannedChange[];
}

interface PlannedChange {
  file: string;
  field: string;
  previous_value: string | null;
  next_value: string | null;
  risk: string | null;
}

interface ApplyResult {
  snapshot: {
    id: string;
  };
}

interface HealthCheckResult {
  status:
    | "healthy"
    | "dns_failure"
    | "tls_failure"
    | "authentication_failure"
    | "timeout"
    | "http_failure";
  elapsed_ms: number;
  message: string | null;
}

type ProfileKind = "official" | "mirror" | "custom";
type TargetScope = "user" | "project";

const scopeLabels: Record<ConfigScope, string> = {
  user: "用户级",
  project: "项目级",
  environment: "环境变量",
};

const profileDefinitions: Record<
  Exclude<ProfileKind, "custom">,
  { id: string; name: string; registry: string; description: string }
> = {
  official: {
    id: "npm-official",
    name: "官方源",
    registry: "https://registry.npmjs.org/",
    description: "npm 官方 registry",
  },
  mirror: {
    id: "npmmirror",
    name: "公共镜像",
    registry: "https://registry.npmmirror.com/",
    description: "候选公共镜像，应用前建议检查连通性",
  },
};

function App() {
  const [projectDirectory, setProjectDirectory] = useState("");
  const [result, setResult] = useState<NpmReadResult | null>(null);
  const [profileKind, setProfileKind] = useState<ProfileKind>("official");
  const [targetScope, setTargetScope] = useState<TargetScope>("user");
  const [customRegistry, setCustomRegistry] = useState("");
  const [plan, setPlan] = useState<ChangePlan | null>(null);
  const [snapshotId, setSnapshotId] = useState<string | null>(null);
  const [healthTarget, setHealthTarget] = useState("");
  const [healthResult, setHealthResult] = useState<HealthCheckResult | null>(
    null,
  );
  const [scanState, setScanState] = useState<"idle" | "loading" | "error">(
    "idle",
  );
  const [errorMessage, setErrorMessage] = useState("");

  const configEntries = Object.entries(result?.effective_config.values ?? {});

  async function scan() {
    setScanState("loading");
    setErrorMessage("");

    try {
      const scanResult = await invoke<NpmReadResult>("scan_npm", {
        projectDirectory: projectDirectory.trim() || null,
      });
      setResult(scanResult);
      setPlan(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function previewProfile() {
    const selectedProfile =
      profileKind === "custom"
        ? {
            id: "custom-registry",
            name: "自定义源",
            registry: customRegistry.trim(),
          }
        : profileDefinitions[profileKind];

    setScanState("loading");
    setErrorMessage("");
    try {
      const [scanResult, previewResult] = await Promise.all([
        invoke<NpmReadResult>("scan_npm", {
          projectDirectory: projectDirectory.trim() || null,
        }),
        invoke<ChangePlan>("preview_npm_profile", {
          request: {
            projectDirectory: projectDirectory.trim() || null,
            targetScope,
            profile: selectedProfile,
          },
        }),
      ]);
      setResult(scanResult);
      setPlan(previewResult);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function applyPreview() {
    if (
      !plan ||
      !window.confirm(
        "确认应用此预览？MirrorIt 将创建本地快照后修改目标 .npmrc。",
      )
    ) {
      return;
    }

    setScanState("loading");
    setErrorMessage("");
    try {
      const applied = await invoke<ApplyResult>("apply_npm_preview", {
        planId: plan.id,
      });
      const scanResult = await invoke<NpmReadResult>("scan_npm", {
        projectDirectory: projectDirectory.trim() || null,
      });
      setResult(scanResult);
      setSnapshotId(applied.snapshot.id);
      setPlan(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function rollbackSnapshot() {
    if (
      !snapshotId ||
      !window.confirm("确认从此快照恢复 npm 配置？当前目标文件将被替换。")
    ) {
      return;
    }

    setScanState("loading");
    setErrorMessage("");
    try {
      await invoke("rollback_npm_snapshot", { snapshotId });
      const scanResult = await invoke<NpmReadResult>("scan_npm", {
        projectDirectory: projectDirectory.trim() || null,
      });
      setResult(scanResult);
      setSnapshotId(null);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  async function checkHealth() {
    setScanState("loading");
    setErrorMessage("");
    try {
      const result = await invoke<HealthCheckResult>("check_npm_health", {
        address: healthTarget.trim(),
      });
      setHealthResult(result);
      setScanState("idle");
    } catch (error) {
      setScanState("error");
      setErrorMessage(String(error));
    }
  }

  return (
    <ThemeProvider>
      <div className="min-h-svh bg-background text-foreground">
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

        <div className="mx-auto grid w-full max-w-7xl grid-cols-[12.5rem_minmax(0,1fr)]">
          <aside className="min-h-[calc(100svh-3.5rem)] border-r border-border px-3 py-5">
            <p className="px-2 text-xs font-medium text-muted-foreground">
              工具
            </p>
            <nav aria-label="工具导航" className="mt-2">
              <a
                aria-current="page"
                className="flex h-8 items-center gap-2 rounded-md bg-primary/10 px-2 text-sm font-medium text-primary"
                href="#npm"
              >
                <Terminal aria-hidden="true" className="size-3.5" />
                npm
              </a>
            </nav>
            <p className="mt-8 px-2 text-xs font-medium text-muted-foreground">
              即将支持
            </p>
            <p className="mt-2 px-2 text-xs leading-5 text-muted-foreground">
              Maven 与 Flutter/Pub 将按相同的安全流程接入。
            </p>
          </aside>

          <main id="npm" className="min-w-0 px-8 py-8">
            <section
              aria-labelledby="npm-heading"
              className="flex flex-wrap items-end justify-between gap-5 border-b border-border pb-7"
            >
              <div>
                <p className="text-xs font-medium text-muted-foreground">
                  工具配置 / 只读扫描
                </p>
                <h1
                  id="npm-heading"
                  className="mt-1 flex items-center gap-2 text-xl font-semibold"
                >
                  npm 配置来源
                  <span className="rounded-full border border-border px-2 py-0.5 text-xs font-medium text-muted-foreground">
                    仅检测
                  </span>
                </h1>
                <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
                  查看 registry、scope registry
                  与代理的最终生效值；扫描不会改动本机文件。
                </p>
              </div>
              <Button disabled={scanState === "loading"} onClick={scan}>
                {scanState === "loading" ? (
                  <LoaderCircle aria-hidden="true" className="animate-spin" />
                ) : (
                  <RefreshCw aria-hidden="true" />
                )}
                扫描配置
              </Button>
            </section>

            <section
              aria-labelledby="health-heading"
              className="border-b border-border py-6"
            >
              <div className="flex flex-wrap items-end justify-between gap-4">
                <div>
                  <p className="text-xs font-medium text-muted-foreground">
                    显式检查
                  </p>
                  <h2
                    id="health-heading"
                    className="mt-1 text-base font-semibold"
                  >
                    npm 源连通性
                  </h2>
                </div>
                <Button
                  disabled={scanState === "loading" || !healthTarget.trim()}
                  onClick={checkHealth}
                  variant="outline"
                >
                  <Activity aria-hidden="true" />
                  检查连接
                </Button>
              </div>
              <div className="mt-4 grid gap-3 lg:grid-cols-[1fr_auto]">
                <input
                  aria-label="检查地址"
                  className="h-9 rounded-md border border-input bg-card px-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                  onChange={(event) => setHealthTarget(event.target.value)}
                  placeholder="https://registry.npmjs.org/"
                  value={healthTarget}
                />
                {healthResult ? (
                  <p className="self-center text-sm text-muted-foreground">
                    {healthResult.status === "healthy"
                      ? "连接正常"
                      : healthResult.status.replace(/_/g, " ")}{" "}
                    · {healthResult.elapsed_ms} ms
                  </p>
                ) : null}
              </div>
            </section>

            <section
              aria-label="扫描范围"
              className="grid gap-3 border-b border-border py-5 lg:grid-cols-[1fr_auto]"
            >
              <label className="grid gap-1.5 text-sm font-medium">
                <span>项目目录（可选）</span>
                <div className="relative">
                  <FolderSearch
                    aria-hidden="true"
                    className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
                  />
                  <input
                    className="h-9 w-full rounded-md border border-input bg-card py-1 pr-3 pl-8 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                    onChange={(event) =>
                      setProjectDirectory(event.target.value)
                    }
                    placeholder="例如 C:\work\my-project"
                    value={projectDirectory}
                  />
                </div>
              </label>
              <p className="self-end pb-2 text-xs text-muted-foreground">
                未填写时只读取用户级和环境变量配置
              </p>
            </section>

            <section
              aria-labelledby="profile-heading"
              className="border-b border-border py-6"
            >
              <div className="flex flex-wrap items-end justify-between gap-4">
                <div>
                  <p className="text-xs font-medium text-muted-foreground">
                    下一步
                  </p>
                  <h2
                    id="profile-heading"
                    className="mt-1 text-base font-semibold"
                  >
                    预览配置档
                  </h2>
                </div>
                <Button
                  disabled={scanState === "loading"}
                  onClick={previewProfile}
                  variant="outline"
                >
                  <FileDiff aria-hidden="true" />
                  查看差异
                </Button>
              </div>

              <div className="mt-4 grid gap-3 lg:grid-cols-3">
                {(
                  Object.keys(profileDefinitions) as Array<
                    Exclude<ProfileKind, "custom">
                  >
                ).map((kind) => {
                  const profile = profileDefinitions[kind];
                  const selected = profileKind === kind;
                  return (
                    <button
                      aria-pressed={selected}
                      className={
                        selected
                          ? "min-h-24 border border-primary bg-primary/5 p-3 text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/30"
                          : "min-h-24 border border-border bg-card p-3 text-left outline-none transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"
                      }
                      key={kind}
                      onClick={() => setProfileKind(kind)}
                      type="button"
                    >
                      <p className="text-sm font-medium">{profile.name}</p>
                      <code className="mt-2 block truncate font-mono text-xs text-muted-foreground">
                        {profile.registry}
                      </code>
                      <p className="mt-2 text-xs leading-4 text-muted-foreground">
                        {profile.description}
                      </p>
                    </button>
                  );
                })}
                <button
                  aria-pressed={profileKind === "custom"}
                  className={
                    profileKind === "custom"
                      ? "min-h-24 border border-primary bg-primary/5 p-3 text-left outline-none focus-visible:ring-3 focus-visible:ring-ring/30"
                      : "min-h-24 border border-border bg-card p-3 text-left outline-none transition-colors hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"
                  }
                  onClick={() => setProfileKind("custom")}
                  type="button"
                >
                  <p className="text-sm font-medium">自定义源</p>
                  <p className="mt-2 text-xs leading-4 text-muted-foreground">
                    使用未含凭据的 HTTPS registry 地址。
                  </p>
                </button>
              </div>

              <div className="mt-4 grid gap-3 lg:grid-cols-2">
                {profileKind === "custom" ? (
                  <label className="grid gap-1.5 text-sm font-medium">
                    <span>自定义 registry</span>
                    <input
                      className="h-9 rounded-md border border-input bg-card px-3 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                      onChange={(event) =>
                        setCustomRegistry(event.target.value)
                      }
                      placeholder="https://registry.example.com/"
                      value={customRegistry}
                    />
                  </label>
                ) : (
                  <div className="grid content-end text-sm">
                    <p className="text-muted-foreground">
                      预览只生成差异，不会修改任何配置文件。
                    </p>
                  </div>
                )}
                <label className="grid gap-1.5 text-sm font-medium">
                  <span>目标作用域</span>
                  <select
                    className="h-9 rounded-md border border-input bg-card px-3 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
                    onChange={(event) =>
                      setTargetScope(event.target.value as TargetScope)
                    }
                    value={targetScope}
                  >
                    <option value="user">用户级 .npmrc</option>
                    <option value="project">项目级 .npmrc</option>
                  </select>
                </label>
              </div>
            </section>

            {scanState === "error" ? (
              <section
                aria-live="polite"
                className="mt-6 flex gap-3 border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm"
              >
                <CircleAlert
                  aria-hidden="true"
                  className="mt-0.5 size-4 shrink-0 text-destructive"
                />
                <div>
                  <p className="font-medium">无法完成扫描</p>
                  <p className="mt-1 text-muted-foreground">{errorMessage}</p>
                </div>
              </section>
            ) : null}

            {!result && scanState !== "loading" ? (
              <section className="grid min-h-64 place-items-center border-b border-border py-10 text-center">
                <div>
                  <PackageSearch
                    aria-hidden="true"
                    className="mx-auto size-6 text-muted-foreground"
                  />
                  <p className="mt-3 text-sm font-medium">尚未扫描 npm 配置</p>
                  <p className="mt-1 text-sm text-muted-foreground">
                    结果会按实际优先级保留来源轨迹。
                  </p>
                </div>
              </section>
            ) : null}

            {result ? (
              <section aria-labelledby="effective-heading" className="pt-7">
                <div className="flex items-baseline justify-between">
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      扫描结果
                    </p>
                    <h2
                      id="effective-heading"
                      className="mt-1 text-base font-semibold"
                    >
                      生效配置
                    </h2>
                  </div>
                  <span className="text-xs text-muted-foreground">
                    {configEntries.length} 项已识别
                  </span>
                </div>

                {configEntries.length ? (
                  <div className="mt-4 divide-y divide-border border-y border-border">
                    {configEntries.map(([key, value]) => (
                      <article
                        className="grid gap-4 py-4 lg:grid-cols-[10rem_minmax(0,1fr)]"
                        key={key}
                      >
                        <div>
                          <p className="font-mono text-sm font-medium">{key}</p>
                          <p className="mt-1 text-xs text-muted-foreground">
                            最终生效值
                          </p>
                        </div>
                        <div className="min-w-0">
                          <code className="block truncate rounded-md bg-muted px-2.5 py-2 font-mono text-xs text-foreground">
                            {value.value ?? "未设置"}
                          </code>
                          <div className="mt-4 flex items-center gap-2">
                            <span className="h-px w-5 bg-border" />
                            <p className="text-xs font-medium text-muted-foreground">
                              来源轨迹
                            </p>
                          </div>
                          <ol className="mt-3 grid gap-2">
                            {value.sources.map((source, index) => (
                              <li
                                className="grid gap-2 border-l-2 border-border py-1.5 pl-3 text-xs"
                                key={`${source.location}-${source.priority}-${index}`}
                              >
                                <div className="flex flex-wrap items-center gap-2">
                                  <span className="font-medium">
                                    {scopeLabels[source.scope]} ·{" "}
                                    {source.location}
                                  </span>
                                  <span
                                    className={
                                      index === value.sources.length - 1
                                        ? "rounded-full bg-primary/10 px-1.5 py-0.5 font-medium text-primary"
                                        : "rounded-full bg-muted px-1.5 py-0.5 text-muted-foreground"
                                    }
                                  >
                                    {index === value.sources.length - 1
                                      ? "最终生效"
                                      : "被后续来源覆盖"}
                                  </span>
                                  {source.sensitive ? (
                                    <span className="inline-flex items-center gap-1 text-warning">
                                      <ShieldCheck
                                        aria-hidden="true"
                                        className="size-3"
                                      />
                                      凭据已掩盖
                                    </span>
                                  ) : null}
                                </div>
                                <code className="block truncate font-mono text-muted-foreground">
                                  {source.value ?? "未设置"}
                                </code>
                              </li>
                            ))}
                          </ol>
                        </div>
                      </article>
                    ))}
                  </div>
                ) : (
                  <div className="mt-4 border-y border-border py-8 text-sm text-muted-foreground">
                    未发现受支持的 npm 配置项。
                  </div>
                )}

                {result.diagnostics.length ? (
                  <section
                    aria-labelledby="diagnostic-heading"
                    className="mt-6 border-l-2 border-warning bg-warning/5 px-4 py-3"
                  >
                    <h3 id="diagnostic-heading" className="text-sm font-medium">
                      需要注意
                    </h3>
                    <ul className="mt-2 grid gap-1 text-sm text-muted-foreground">
                      {result.diagnostics.map((diagnostic) => (
                        <li key={diagnostic}>{diagnostic}</li>
                      ))}
                    </ul>
                  </section>
                ) : null}
              </section>
            ) : null}

            {plan ? (
              <section aria-labelledby="plan-heading" className="mt-8">
                <div className="flex flex-wrap items-end justify-between gap-3">
                  <div>
                    <p className="text-xs font-medium text-muted-foreground">
                      只读预览
                    </p>
                    <h2
                      id="plan-heading"
                      className="mt-1 text-base font-semibold"
                    >
                      将要发生的变更
                    </h2>
                  </div>
                  <div className="flex items-center gap-3">
                    <span className="text-xs text-muted-foreground">
                      {plan.changes.length} 项字段
                    </span>
                    <Button
                      disabled={scanState === "loading"}
                      onClick={applyPreview}
                    >
                      <Save aria-hidden="true" />
                      确认应用
                    </Button>
                  </div>
                </div>
                <div className="mt-4 divide-y divide-border border-y border-border">
                  {plan.changes.map((change) => (
                    <article
                      className="py-4"
                      key={`${change.file}-${change.field}`}
                    >
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <p className="font-mono text-sm font-medium">
                          {change.field}
                        </p>
                        <p className="truncate font-mono text-xs text-muted-foreground">
                          {change.file}
                        </p>
                      </div>
                      <div className="mt-3 grid gap-2 text-xs md:grid-cols-2">
                        <div className="rounded-md bg-muted px-2.5 py-2">
                          <p className="mb-1 text-muted-foreground">当前值</p>
                          <code className="block truncate font-mono">
                            {change.previous_value ?? "未设置"}
                          </code>
                        </div>
                        <div className="rounded-md bg-primary/5 px-2.5 py-2">
                          <p className="mb-1 text-primary">新值</p>
                          <code className="block truncate font-mono">
                            {change.next_value ?? "移除"}
                          </code>
                        </div>
                      </div>
                      {change.risk ? (
                        <p className="mt-3 border-l-2 border-warning pl-2 text-xs text-warning">
                          {change.risk}
                        </p>
                      ) : null}
                    </article>
                  ))}
                </div>
              </section>
            ) : null}

            {snapshotId ? (
              <section className="mt-6 flex flex-wrap items-center justify-between gap-3 border-l-2 border-primary bg-primary/5 px-4 py-3">
                <div>
                  <p className="text-sm font-medium">已创建可恢复快照</p>
                  <p className="mt-1 font-mono text-xs text-muted-foreground">
                    {snapshotId}
                  </p>
                </div>
                <Button
                  disabled={scanState === "loading"}
                  onClick={rollbackSnapshot}
                  variant="outline"
                >
                  <RotateCcw aria-hidden="true" />
                  从快照恢复
                </Button>
              </section>
            ) : null}
          </main>
        </div>
      </div>
    </ThemeProvider>
  );
}

export default App;
