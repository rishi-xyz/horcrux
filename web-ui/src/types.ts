export interface DriveInfo {
  name: string;
  mount_point: string;
  total_bytes: number;
  available_bytes: number;
}

export interface LogEntry {
  ts: number;
  ts_utc: string;
  attempt: number;
  shard_id: number | null;
  kind: "decrypt_ok" | "decrypt_fail" | "blocked" | "signed";
}

export interface VerifyReport {
  path: string;
  ok: boolean;
  kind: "sss" | "frost" | null;
  threshold: number | null;
  shares: number | null;
}

export interface SignResult {
  from: string;
  signature?: string;
  raw_base58?: string;
  raw_hex?: string;
  txid?: string;
  fee_sat?: number;
  change_sat?: number;
  mined_signature?: string;
  broadcast_error?: string;
  warnings: string[];
}

export interface BlockedError {
  reasons: string[];
}

/// One guardian's chosen destination for a split (see `GuardianDestRow`):
/// this device (the form's shared `out_dir`), a detected removable drive,
/// or a typed custom path.
export type DestChoice =
  | { kind: "this-device" }
  | { kind: "drive"; index: number }
  | { kind: "custom"; path: string };

export interface Guardian {
  dest: DestChoice;
  password: string;
}
