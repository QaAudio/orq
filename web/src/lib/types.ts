export type FilterMode = "active" | "all" | "archived";

export type PoiRow = {
  key?: string;
  state?: string;
  value?: unknown;
  version?: number | string;
  updated_at?: string;
  columns?: Record<string, unknown>;
  blocked?: boolean;
  table?: string;
  blocker_reason?: string;
};

export type TaskRow = {
  id?: string;
  name?: string;
  status?: string;
  model_id?: string;
  profile?: string;
  command?: string;
  depends_on?: string[];
  claims?: string[];
  needs_poi?: string[];
  session?: string | null;
  job_id?: string | null;
  pid?: number | null;
  attempt?: number;
  max_attempts?: number;
  exit_code?: number | null;
  started_at?: string;
  finished_at?: string;
  created_at?: string;
  updated_at?: string;
  log_path?: string;
  error?: string;
};

export type PanelId =
  | "ops-health"
  | "running-tasks"
  | "board"
  | "tasks"
  | "jobs"
  | "aff"
  | "events"
  | "files";

export type DockLeaf = {
  tabs: PanelId[];
  active: number;
  height: number;
};

export type DockLayout = {
  v: 1;
  colSplitPct: number;
  columns: [DockLeaf[], DockLeaf[]];
};

export type JobRow = {
  id?: string;
  name?: string;
  status?: string;
  strategy?: string;
  route_reason?: string;
};

export type AffRow = {
  class?: string;
  model_id?: string;
  score?: number;
  n?: number;
};

export type EventRow = {
  id?: number | string;
  kind?: string;
  payload?: unknown;
  created_at?: string;
};

export type LeaseRow = {
  table?: string;
  key?: string;
  kind?: string;
  holder?: string;
  reason?: string;
  expires_at?: string;
};

export type TriggerRow = {
  id?: string;
  name?: string;
  event_pattern?: string;
  enabled?: boolean;
};

export type DashSnapshot = {
  workspace?: string;
  updated?: string;
  static_demo?: boolean;
  board?: PoiRow[];
  tasks?: TaskRow[];
  jobs?: JobRow[];
  affinities?: AffRow[];
  events?: EventRow[];
  files?: string[];
  canvases?: PoiRow[];
  leases?: LeaseRow[];
  triggers?: TriggerRow[];
  blocked_pois?: PoiRow[];
  trigger_failures?: EventRow[];
  active_sessions?: string[];
  models?: { id?: string; display_name?: string }[];
  daemon?: { running?: boolean };
  computer_focus?: PoiRow | null;
};
