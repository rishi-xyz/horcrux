"use client";

export class ApiError extends Error {
  status: number;
  data: any;
  constructor(message: string, status: number, data?: any) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.data = data;
  }
}

let cachedToken: string | null = null;
export function getToken(): string {
  if (cachedToken !== null) return cachedToken;
  if (typeof window === "undefined") return "";
  cachedToken = new URLSearchParams(window.location.search).get("token") || "";
  return cachedToken;
}
export function hasToken(): boolean { return !!getToken(); }

async function request<T>(path: string, init?: RequestInit & { json?: any }): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = {
    ...(init?.headers as Record<string, string> | undefined),
    "X-Horcrux-Token": token,
  };
  let body = init?.body;
  if (init?.json !== undefined) {
    headers["Content-Type"] = "application/json";
    body = JSON.stringify(init.json);
  }
  const res = await fetch(path.startsWith("/api") ? path : `/api${path}`, {
    ...init,
    headers,
    body,
  });
  const text = await res.text();
  let data: any = null;
  try { data = text ? JSON.parse(text) : null; } catch { data = text; }
  if (!res.ok) {
    const msg = data?.error || data?.message || `Request failed (${res.status})`;
    throw new ApiError(typeof msg === "string" ? msg : JSON.stringify(msg), res.status, data);
  }
  return data as T;
}

export const api = {
  get: <T>(path: string) => request<T>(path, { method: "GET" }),
  post: <T>(path: string, json?: any) => request<T>(path, { method: "POST", json }),
  del: <T>(path: string) => request<T>(path, { method: "DELETE" }),
};

export function isBlocked(e: unknown): e is ApiError & { data: { reasons: string[] } } {
  return e instanceof ApiError && e.status === 409 && Array.isArray((e.data as any)?.reasons);
}
