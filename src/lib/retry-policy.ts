/**
 * TanStack Query 重试策略（v0.9a）。
 *
 * 用更智能的策略取代默认的 `retry: 1`：
 *   1. 检查错误的 shape（来自 `lib/invoke-safe.ts`）
 *   2. 遵守我们已经附加的 `retryable` 标记
 *   3. 加入指数退避 + 抖动（避免 10 个查询
 *      同时在瞬间重试）
 *   4. 限制总重试次数，防止猛打后端
 *
 * 行为：
 *   - `retryable: false` 错误（invalid / not_found / internal / serde /
 *     keyring）→ 立即失败，不重试
 *   - `retryable: true` 错误（db / http / io / network / unknown）→
 *     按 500ms、1.5s、4.5s 的间隔重试（×3 退避，±20% 抖动）
 *   - 最多 3 次重试（共 4 次尝试）
 *   - 所有 `retryable:false` 错误在首次失败时
 *     即被推送给 UI
 *
 * 这是 v0.8b → v0.9a 的升级。错误分类器仍是
 * 单一信息源；本文件只是读取其 `retryable` 标记
 * 的重试策略。
 */

import type { QueryClient } from '@tanstack/react-query';
import { canRetry, type AppErrorShape } from './invoke-safe';

const MAX_RETRIES = 3;
const BASE_DELAY_MS = 500;
const BACKOFF_FACTOR = 3;     // 500ms → 1.5s → 4.5s
const JITTER_FRACTION = 0.2;  // ±20%

/**
 * 计算第 `n` 次（从 0 开始）重试之前的延迟。
 *   n=0 → 500ms ± 抖动
 *   n=1 → 1500ms ± 抖动
 *   n=2 → 4500ms ± 抖动
 */
export function retryDelayMs(attempt: number): number {
  const base = BASE_DELAY_MS * Math.pow(BACKOFF_FACTOR, attempt);
  const jitter = base * JITTER_FRACTION * (Math.random() * 2 - 1);
  return Math.round(base + jitter);
}

/**
 * 谓词：该错误是否应被重试？
 * 对于不可重试类型返回 `false`，对于可重试的
 * 错误返回 `true`（最多 MAX_RETRIES 次）。
 */
export function shouldRetry(failureCount: number, error: unknown): boolean {
  if (failureCount >= MAX_RETRIES) return false;
  // 非 shape 错误无法安全分类；按可重试处理。
  if (!isAppErrorShape(error)) return true;
  return canRetry(error);
}

function isAppErrorShape(e: unknown): e is AppErrorShape {
  return (
    typeof e === 'object' &&
    e !== null &&
    'kind' in e &&
    'message' in e &&
    'retryable' in e
  );
}

/**
 * 将 v0.9a 重试策略应用到 QueryClient 实例。
 * 在创建 client 之后于 `main.tsx` 中调用。
 */
export function applyRetryPolicy(client: QueryClient): void {
  client.setDefaultOptions({
    queries: {
      // 冷启动时不要猛打。已经在后台抢占式重试；
      // 仅在窗口聚焦时重新获取。
      refetchOnWindowFocus: false,
      retry: shouldRetry,
      retryDelay: retryDelayMs,
      // 30s 陈旧度在数据新鲜度与
      // 「不要每次渲染都重新拉取」之间是合理的折衷。
      staleTime: 30_000,
    },
    mutations: {
      // mutation 由用户触发；不要自动重试（往往
      // 有副作用）。用户可以再次点击按钮。
      retry: false,
    },
  });
}

/** 最大重试次数（暴露给测试和 UI）。 */
export const MAX_RETRY_ATTEMPTS = MAX_RETRIES;
