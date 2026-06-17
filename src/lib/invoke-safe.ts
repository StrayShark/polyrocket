/**
 * L1 — Unified invoke wrapper with error classification.
 *
 * The Rust `AppError` enum serializes as a string like
 *   "database error: connection refused"
 *   "keyring error: entry not found"
 *   "invalid input: market_id is required"
 *   "not found: wallet wlt-xxx"
 *   "internal: ..."
 *   "http error: ..."
 *   "io error: ..."
 *   "serde error: ..."
 *
 * This wrapper classifies those into a stable `AppErrorShape` that
 * components can switch on (render different UI, decide whether to
 * retry, etc.) instead of regex-parsing error strings.
 *
 * v0.8b — replaces the bare `invoke()` calls scattered across routes
 * with a single safe entry point. TanStack Query catches the throw
 * and stores it in `query.error`, so the new shape flows naturally.
 */

import { invoke as tauriInvoke } from '@tauri-apps/api/core';

/** Coarse classification matching the Rust `AppError` variants. */
export type AppErrorKind =
  | 'db'
  | 'http'
  | 'serde'
  | 'keyring'
  | 'io'
  | 'invalid'
  | 'not_found'
  | 'internal'
  | 'network'
  | 'unknown';

/** A structured, JSON-safe error returned by `safeInvoke`. */
export interface AppErrorShape {
  kind: AppErrorKind;
  message: string;
  /** Best-effort human hint; safe to render in UI. */
  hint: string;
  /** Whether retrying the same call is likely to succeed. */
  retryable: boolean;
  /** Original raw error string from the Rust side. */
  raw: string;
}

const KIND_HINTS: Record<AppErrorKind, string> = {
  db: 'A local database operation failed. Try again, or check disk space / permissions.',
  http: 'A network call to an external service failed. Check your connection or the service status.',
  serde: 'A serialization mismatch between layers. This is a bug — please report it.',
  keyring: 'OS keyring is unavailable. Make sure you have a default keychain (e.g. login.keychain on macOS).',
  io: 'A file-system operation failed. Check disk space and folder permissions.',
  invalid: 'The request was rejected by validation. Check the form and try again.',
  not_found: 'The requested item no longer exists. Refresh and try again.',
  internal: 'An internal error occurred. Please report it with the timestamp.',
  network: 'The Tauri IPC channel itself failed (e.g. window closed mid-call).',
  unknown: 'An unknown error occurred.',
};

const RETRYABLE: Record<AppErrorKind, boolean> = {
  db: true,
  http: true,
  serde: false,
  keyring: false,
  io: true,
  invalid: false,
  not_found: false,
  internal: false,
  network: true,
  unknown: true,
};

/**
 * Classify a raw error string from Rust.
 * Order matters: more specific prefixes first.
 */
export function classifyError(raw: string): AppErrorShape {
  const lower = raw.toLowerCase();
  let kind: AppErrorKind = 'unknown';
  if (lower.startsWith('database error:') || lower.startsWith('sqlx') || lower.includes('sqlite')) {
    kind = 'db';
  } else if (lower.startsWith('http error:') || lower.includes('reqwest') || lower.includes('connection refused')) {
    kind = 'http';
  } else if (lower.startsWith('serde error:')) {
    kind = 'serde';
  } else if (lower.startsWith('keyring error:')) {
    kind = 'keyring';
  } else if (lower.startsWith('io error:')) {
    kind = 'io';
  } else if (lower.startsWith('invalid input:')) {
    kind = 'invalid';
  } else if (lower.startsWith('not found:')) {
    kind = 'not_found';
  } else if (lower.startsWith('internal:')) {
    kind = 'internal';
  } else if (lower.includes('window not found') || lower.includes('ipc failed')) {
    kind = 'network';
  }
  return {
    kind,
    message: raw,
    hint: KIND_HINTS[kind],
    retryable: RETRYABLE[kind],
    raw,
  };
}

/**
 * Thin, type-safe wrapper around `invoke` that:
 *   1. Adds a `cmd` field to the args object (Tauri requires `args` to
 *      be an object, even if empty).
 *   2. Catches errors and re-throws them as an `AppErrorShape` so
 *      TanStack Query's `error` field is structured, not a string.
 *
 * Usage:
 *   const data = await safeInvoke<Market[]>('list_markets', { args: { ... } });
 *   // on failure, `error instanceof AppErrorShape` is true-ish;
 *   // call sites that want a string can use `error.message`.
 */
export async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args ?? {});
  } catch (e) {
    const raw = e instanceof Error ? e.message : String(e);
    throw classifyError(raw);
  }
}

/** Render-friendly summary: `${kind} · ${message}`. */
export function formatError(e: AppErrorShape): string {
  return `${e.kind}: ${e.message}`;
}

/** True if the error is one the user can fix by retrying. */
export function canRetry(e: AppErrorShape): boolean {
  return e.retryable;
}
