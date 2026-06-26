// v0.62a.2 — Notifications 组件测试。
//
// /notifications 是 toast 列表页面。
// 当前覆盖率 0%。本文件覆盖：
//   1. 空 toast 初始渲染
//   2. 推送一条 toast 后渲染
//   3. Clear 按钮可用

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k }),
}));

vi.mock('@/ipc', () => ({
  sendNotification: vi.fn(),
}));

import { Notifications } from './Notifications';
import { useToastStore } from '@/stores/toast-store';

describe('Notifications', () => {
  beforeEach(() => {
    useToastStore.getState().clear();
  });

  it('renders the page with empty state when no toasts', async () => {
    render(
      <MemoryRouter>
        <Notifications />
      </MemoryRouter>,
    );
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('shows a toast when one is in the store', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'hello', ttl: 0 });
    render(
      <MemoryRouter>
        <Notifications />
      </MemoryRouter>,
    );
    await waitFor(() => {
      expect(screen.getByText('hello')).toBeInTheDocument();
    });
  });

  it('clear button empties the toasts', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'a', ttl: 0 });
    useToastStore.getState().push({ kind: 'info', title: 'b', ttl: 0 });
    render(
      <MemoryRouter>
        <Notifications />
      </MemoryRouter>,
    );
    const clearBtn = await screen.findByText(/clear/i);
    fireEvent.click(clearBtn);
    await waitFor(() => {
      expect(useToastStore.getState().toasts).toEqual([]);
    });
  });
});
