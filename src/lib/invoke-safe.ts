/**
 * L1 —— 统一的 invoke 包装与错误分类。
 *
 * Rust 的 `AppError` 枚举以字符串形式序列化，例如
 *   "database error: connection refused"
 *   "keyring error: entry not found"
 *   "invalid input: market_id is required"
 *   "not found: wallet wlt-xxx"
 *   "internal: ..."
 *   "http error: ..."
 *   "io error: ..."
 *   "serde error: ..."
 *
 * 该包装将它们分类为稳定的 `AppErrorShape`，
 * 组件可以在其上做 switch（渲染不同的 UI、
 * 决定是否重试等），而不是用正则去解析
 * 错误字符串。
 *
 * v0.8b —— 用一个安全的入口点取代分散在
 * 路由中的裸 `invoke()` 调用。TanStack Query
 * 会捕获 throw 并存储到 `query.error` 中，
 * 因此新的 shape 可以自然地流动。
 */

import { invoke as tauriInvoke } from '@tauri-apps/api/core';

/** 与 Rust `AppError` 变体对应的粗粒度分类。 */
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

/** `safeInvoke` 返回的结构化、JSON 安全的错误。 */
export interface AppErrorShape {
  kind: AppErrorKind;
  message: string;
  /** 尽力而为的人类提示；可直接在 UI 中渲染。 */
  hint: string;
  /** 重试相同调用是否可能成功。 */
  retryable: boolean;
  /** 来自 Rust 端的原始错误字符串。 */
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
 * 对来自 Rust 的原始错误字符串进行分类。
 * 顺序很重要：更具体的前缀优先。
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
 * 围绕 `invoke` 的轻量、类型安全的包装器：
 *   1. 给 args 对象加上 `cmd` 字段（Tauri 要求
 *      `args` 必须是一个对象，即使为空）。
 *   2. 捕获错误并以 `AppErrorShape` 重新抛出，使
 *      TanStack Query 的 `error` 字段是结构化的，
 *      而不是一个字符串。
 *
 * 用法：
 *   const data = await safeInvoke<Market[]>('list_markets', { args: { ... } });
 *   // 失败时，`error instanceof AppErrorShape` 近似为 true；
 *   // 想要字符串形式的调用方可以使用 `error.message`。
 */
export async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args ?? {});
  } catch (e) {
    const raw = e instanceof Error ? e.message : String(e);
    throw classifyError(raw);
  }
}

/** 适合直接渲染的摘要形式：${kind}: ${message} */
export function formatError(e: AppErrorShape): string {
  return `${e.kind}: ${e.message}`;
}

/** 如果该错误可通过重试解决则返回 true。 */
export function canRetry(e: AppErrorShape): boolean {
  return e.retryable;
}
