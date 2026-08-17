export interface ToolInfo {
  id: string;
  name: string;
  path: string;
}

export interface Environment {
  tmux: string | null;
  terminals: ToolInfo[];
  agents: ToolInfo[];
}

export interface CustomAgent {
  name: string;
  command: string;
}

export interface Config {
  default_terminal: string;
  default_agent: string;
  default_panes: number;
  custom_agent?: CustomAgent;
  recent_dirs: string[];
  use_standard_claude: boolean;
  panel_bypass_permissions?: boolean;
  desktop_notifications?: boolean;
}

export type ManagedClaudeState =
  | "not-installed"
  | "needs-repair"
  | "healthy"
  | "unavailable";

export interface ManagedClaudeStatus {
  state: ManagedClaudeState;
  version: string;
  path?: string;
  standardClaudeAvailable: boolean;
  usingStandard: boolean;
}

/** Which Claude mode an action switches to. Install/repair are the "managed" path. */
export type ClaudeMode = "managed" | "standard";

/** The one-line nudge shown when Claude cannot yet use managed comms. */
export type ClaudeHint = "install" | "repair" | null;

/**
 * Managed Claude only earns a line of the Create dialog when something is
 * actually wrong and the user has not already chosen Standard on purpose.
 * Everything else stays silent.
 */
export function claudeHint(status: ManagedClaudeStatus | null): ClaudeHint {
  if (!status || status.state === "unavailable" || status.usingStandard) return null;
  if (status.state === "needs-repair") return "repair";
  if (status.state === "not-installed") return "install";
  return null;
}

/**
 * The quiet mode switch tucked into the Claude chip. It is null whenever
 * `claudeHint` has something to say, so the two never compete for attention.
 */
export function claudeSwitchTarget(status: ManagedClaudeStatus | null): ClaudeMode | null {
  if (!status || status.state === "unavailable") return null;
  if (status.usingStandard) return "managed";
  if (status.state !== "healthy") return null;
  return status.standardClaudeAvailable ? "standard" : null;
}

/** Mirrors `CreateOpts` in src-tauri/src/models.rs (serde snake_case fields). */
export interface CreateOpts {
  name: string;
  dir: string | null;
  agent_id: string;
  /** Per-pane Agent ids; must be empty or exactly `panes` long. */
  pane_agent_ids: string[];
  panes: number;
  terminal_id: string;
}

export interface TmuxPane {
  id: string;
  command: string;
  active: boolean;
  content?: string;
  session_target?: string;
  slot?: string | null;
  attached?: boolean;
  /** Agent this pane was launched with; authoritative over matching `command`. */
  agent_id?: string;
}

export interface TmuxSession {
  id: string;
  name: string;
  windows_count: number;
  panes_count: number;
  attached: boolean;
  created_at: string;
  last_active_ts: number;
  panes: TmuxPane[];
  native_split?: boolean;
  terminal_id?: string;
}

/** Mirrors `AgentUsage` in src-tauri/src/usage.rs (serde camelCase). */
export interface AgentUsage {
  agentId: string;
  displayName: string;
  todayTokens: number;
  tokens30d: number;
  sessions30d: number;
  lastActiveTs: number | null;
  /** False when the agent's local logs aren't present — render an empty state, not a zero. */
  available: boolean;
}

/** Mirrors `UsageSnapshot` in src-tauri/src/usage.rs. `updatedAt === 0` means the first collection is still running. */
export interface UsageSnapshot {
  agents: AgentUsage[];
  totalToday: number;
  total30d: number;
  updatedAt: number;
  elapsedMs: number;
}

export interface BridgePairingStatus {
  enabled: boolean;
  port: number;
  httpUrl?: string;
  wsUrl?: string;
  httpUrls?: string[];
  wsUrls?: string[];
  token: string;
  connectedClients: number;
  brokerConnected: boolean;
  trustedLanOnly?: boolean;
}

export type AdapterHealthState =
  | "healthy"
  | "healthy-existing-global"
  | "not-installed"
  | "needs-upgrade"
  | "needs-repair"
  | "incompatible-namespace"
  | "unavailable";

export type CommunicationAdapterKind =
  | "pi-extension"
  | "claude-plugin-monitor"
  | "codex-mcp"
  | "opencode-plugin";

export type AdapterSourceKind =
  | "bundled"
  | "npm-registry"
  | "pi-git"
  | "existing-global";

export type CanonicalAdapterPackage =
  | "@ctliz/agent-intercom-pi"
  | "@ctliz/agent-intercom-claude"
  | "@ctliz/agent-intercom-codex"
  | "@ctliz/agent-intercom-opencode";

export type ConfigChangeKind =
  | "none"
  | "app-private-managed"
  | "host-config-registered";

export type AdapterActionReason =
  | "install"
  | "upgrade"
  | "repair"
  | "manual-migration-required";

export interface CommunicationAdapterPlanItem {
  agentId: string;
  hostDisplayName: string;
  adapterKind: CommunicationAdapterKind;
  state: AdapterHealthState;
  targetVersion: string;
  installedVersion: string | null;
  sourceKind: AdapterSourceKind;
  packageName?: CanonicalAdapterPackage;
  configChangeKind: ConfigChangeKind;
  networkRequired: boolean;
  license: string;
  actionReason: AdapterActionReason;
}

export interface WorkspaceInstallPlan {
  /** Opaque server plan identifier */
  planId: string;
  /** Opaque server plan fingerprint */
  planFingerprint: string;
  requiresConsent: boolean;
  canApply: boolean;
  canCreateWithoutInstalling: boolean;
  healthyAgentIds: string[];
  items: CommunicationAdapterPlanItem[];
}

export type AdapterConsentAction =
  | "install-and-create"
  | "create-without-installing"
  | "cancel";

export type TeamRole = "lead" | "worker";

/**
 * Reorders a pane agent array so that the designated Lead agent is placed at index 0 (Pane 1).
 * Preserves the overall count and relative order of coworker agents.
 * Returns a new array copy; does not mutate the input array.
 */
export function reorderPaneAgentsForLead(
  paneAgentIds: string[],
  leadIndex: number
): string[] {
  if (
    !Number.isInteger(leadIndex) ||
    leadIndex <= 0 ||
    leadIndex >= paneAgentIds.length
  ) {
    return [...paneAgentIds];
  }
  const leadAgent = paneAgentIds[leadIndex];
  const rest = paneAgentIds.filter((_, idx) => idx !== leadIndex);
  return [leadAgent, ...rest];
}
