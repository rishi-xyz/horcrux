// Every /api/* request needs the per-run bearer token the Rust server
// printed at startup and pre-filled into this page's URL (?token=...).
// Read once at module load — the token never changes for the life of a
// server process, so there's no need to re-read it on every call.
const TOKEN = new URLSearchParams(window.location.search).get("token") || "";

export function hasToken(): boolean {
  return TOKEN.length > 0;
}

export class ApiError extends Error {
  status?: number;
  data?: unknown;
  constructor(message: string, status?: number, data?: unknown) {
    super(message);
    this.status = status;
    this.data = data;
  }
}

export async function api<T = unknown>(
  path: string,
  options?: { method?: string; body?: unknown }
): Promise<T> {
  const res = await fetch("/api" + path, {
    method: options?.method || "GET",
    headers: {
      "Content-Type": "application/json",
      "X-Horcrux-Token": TOKEN,
    },
    body: options?.body ? JSON.stringify(options.body) : undefined,
  });
  let data: unknown = null;
  try {
    data = await res.json();
  } catch {
    data = null;
  }
  if (!res.ok) {
    const message =
      (data && typeof data === "object" && "error" in data
        ? String((data as { error: unknown }).error)
        : undefined) || res.statusText;
    throw new ApiError(message, res.status, data);
  }
  return data as T;
}
