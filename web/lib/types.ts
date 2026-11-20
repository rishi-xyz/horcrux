export type Verdict = "allow" | "warn" | "block";

export interface LogEntry {
  ts: number;
  ts_utc?: string;
  attempt: number;
  shard_id: number | null;
  kind: "decrypt_ok" | "decrypt_fail" | "blocked" | "signed";
}

export interface VerifyReport {
  path: string;
  ok: boolean;
  kind: "sss" | "frost" | "btc_frost" | null;
  threshold: number | null;
  shares: number | null;
  message?: string;
}

export interface SignResult {
  chain?: string;
  from?: string;
  signature?: string;
  raw_base58?: string;
  raw_hex?: string;
  txid?: string;
  fee_sat?: number;
  change_sat?: number;
  mined_signature?: string | null;
  broadcast_error?: string | null;
  warnings?: string[];
  event_id?: string;
  ai?: AiEventRef | null;
}

export interface DriveInfo {
  name: string;
  mount_point: string;
  total_bytes?: number;
  available_bytes?: number;
  removable?: boolean;
}

export interface AiStatus {
  configured: boolean;
  model: string;
  source: "env" | "dotenv" | "none" | string;
  key_redacted?: string | null;
  background_enabled: boolean;
}

export interface AiEventRef {
  id: string;
  ts: number;
  ts_utc?: string;
  action: string;
  derived?: string;
  status: "pending" | "ok" | "anomaly" | "unavailable" | "skipped" | string;
  summary?: string | null;
  model?: string | null;
  severity?: "info" | "warn" | "critical" | string;
  deterministic?: { verdict?: string; reasons?: string[] } | null;
  details?: Record<string, unknown> | null;
  // compatibility fields some UIs may render
  event_type?: string;
  chain?: string;
  context?: Record<string, unknown> | null;
}

export interface WalletProfile {
  id: string;
  label: string;
  type: "shard_backed" | "browser_connected" | "watch_only" | string;
  // backend currently serializes as `type`, but keep compatibility
  kind?: "shard_backed" | "browser_connected" | "watch_only" | string;
  chain: "solana" | "cosmos" | string;
  network: string; // solana: localnet|devnet|custom, cosmos: local|custom
  address: string;
  protected: boolean;
  created_at: number;
  source?: string | null;
  shard_refs?: string[] | null;
  group_ref?: string | null;
  notes?: string | null;
}

export interface BalanceResult {
  chain: string;
  network: string;
  address: string;
  balance: string | null;
  unit: string;
  sol?: number;
  note?: string;
  rpc_url?: string;
  raw?: unknown;
}

export interface ChainTx {
  signature?: string;
  hash?: string;
  slot?: number;
  height?: number;
  timestamp?: number | null;
  status?: string;
  err?: unknown;
  fee?: number | string | null;
  memo?: string | null;
  direction?: "in" | "out" | "unknown";
  amount?: string | null;
  raw?: unknown;
}

export interface StatusResponse {
  ok: boolean;
  version: string;
  bind: string;
  token_required?: boolean;
  ai: AiStatus;
  defaults: Record<string, string>;
}
