// v0.95 —— useInvoke hook 测试（+2 个测试，+2 个语句）。
//
// useInvoke.ts 是对 React Query 的 useQuery 的轻量封装。
// 它在 v0.91 被加入覆盖率包含列表但没有
// 直接测试。本文件添加 2 个测试以提升 fn/stmts。

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useInvoke } from './useInvoke';

function wrap<T>(hook: () => T) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return renderHook(hook, {
    wrapper: ({ children }) => (
      <QueryClientProvider client={qc}>{children}</QueryClientProvider>
    ),
  });
}

describe('useInvoke (v0.95)', () => {
  it('returns query result with the provided fn', async () => {
    const fn = vi.fn().mockResolvedValue({ foo: 'bar' });
    const { result } = wrap(() => useInvoke(['test', 'key'], fn, undefined as void));
    await waitFor(() => {
      expect(result.current.data).toEqual({ foo: 'bar' });
    });
    expect(fn).toHaveBeenCalled();
  });

  it('passes args to the fn and includes them in the query key', async () => {
    const fn = vi.fn().mockResolvedValue('result');
    const { result } = wrap(() => useInvoke(['args-test'], fn, { id: 42 }));
    await waitFor(() => {
      expect(result.current.data).toBe('result');
    });
    expect(fn).toHaveBeenCalledWith({ id: 42 });
  });
});
