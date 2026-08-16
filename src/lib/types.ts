export type ConfigScope =
  "system" | "user" | "project" | "virtual_environment" | "environment";

export interface ConfigSource {
  scope: ConfigScope;
  location: string;
  priority: number;
  sensitive: boolean;
  value: string | null;
}

export interface EffectiveValue {
  value: string | null;
  sources: ConfigSource[];
}

export interface ToolReadResult {
  effective_config: {
    values: Record<string, EffectiveValue>;
  };
  diagnostics: string[];
}

export interface ChangePlan {
  id: string;
  target_checksums: Record<string, string>;
  file_checksums: Record<string, string>;
  changes: PlannedChange[];
}

export interface PlannedChange {
  file: string;
  field: string;
  previous_value: string | null;
  next_value: string | null;
  risk: string | null;
}

export interface ApplyResult {
  snapshot: {
    id: string;
  };
}

export interface HealthCheckResult {
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

export interface ProfileExportDocument {
  format: "mirrorit-profile";
  version: number;
  profiles: Array<{
    tool: "npm";
    id: string;
    name: string;
    values: Record<string, string>;
  }>;
}

export interface NpmProfileImportPreview {
  id: string;
  name: string;
  current_registry: string;
  imported_registry: string;
  changed: boolean;
}

export type ProfileKind = "official" | "mirror" | "custom";
export type TargetScope = "user" | "project";

export interface NpmProfileSelection {
  id: string;
  name: string;
  registry: string;
}
