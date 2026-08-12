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
