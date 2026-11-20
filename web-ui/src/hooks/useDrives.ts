import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { DriveInfo } from "../types";

/// Detected removable drives (`GET /api/drives`, same `sysinfo`-based
/// detection as the TUI), for the guardian destination picker. Fetched once
/// on mount; call the returned `rescan` after plugging in a drive.
export function useDrives() {
  const [drives, setDrives] = useState<DriveInfo[]>([]);

  const rescan = useCallback(async () => {
    try {
      const res = await api<{ drives: DriveInfo[] }>("/drives");
      setDrives(res.drives || []);
    } catch {
      setDrives([]);
    }
  }, []);

  useEffect(() => {
    void rescan();
  }, [rescan]);

  return { drives, rescan };
}
