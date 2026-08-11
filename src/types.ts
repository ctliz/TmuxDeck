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

export interface TmuxPane {
  id: string;
  command: string;
  active: boolean;
  content?: string;
  session_target?: string;
  slot?: string | null;
  attached?: boolean;
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
}
