// v0.89d —— Notifications.tsx 分支轮次（+7 个测试，br 66.66→100%）。
//
// 4 个 kind 分支（info/success/warning/error）以及 body
// 存在/不存在的分支位于 src/routes/Notifications.tsx:78-86，
// 是当前的覆盖空白。已有 Notifications.test.tsx 仅覆盖了
// 不带 body 的 'info' kind。本文件新增：
//   - success / warning / error / info kind 的渲染（Pill class）
//   - body 存在分支（渲染 body 文本）
//   - body 不存在分支（无 .text-fg-secondary 元素）
//   - dismiss 按钮移除单条 toast

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

describe('Notifications — v0.89d branches', () => {
  beforeEach(() => {
    useToastStore.getState().clear();
  });

  it('renders success toast with bull background (KIND_CLS[success] branch)', async () => {
    useToastStore.getState().push({ kind: 'success', title: 'win', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('win')).toBeInTheDocument();
    });
    expect(document.querySelector('.bg-bull\\/10')).toBeInTheDocument();
  });

  it('renders error toast with bear background (KIND_CLS[error] branch)', async () => {
    useToastStore.getState().push({ kind: 'error', title: 'fail', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('fail')).toBeInTheDocument();
    });
    expect(document.querySelector('.bg-bear\\/10')).toBeInTheDocument();
  });

  it('renders warning toast with warning background (KIND_CLS[warning] branch)', async () => {
    useToastStore.getState().push({ kind: 'warning', title: 'careful', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('careful')).toBeInTheDocument();
    });
    expect(document.querySelector('.bg-warning\\/10')).toBeInTheDocument();
  });

  it('renders info toast with accent background (KIND_CLS[info] branch)', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'fyi', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('fyi')).toBeInTheDocument();
    });
    expect(document.querySelector('.bg-accent\\/10')).toBeInTheDocument();
  });

  it('renders toast body when body is present (t2.body truthy branch)', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'with body', body: 'extra details', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('extra details')).toBeInTheDocument();
    });
  });

  it('omits body element when body is not provided (t2.body falsy branch)', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'no body here', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('no body here')).toBeInTheDocument();
    });
    // body 不应渲染 .text-fg-secondary 元素
    const bodies = document.querySelectorAll('.text-fg-secondary');
    expect(bodies.length).toBe(0);
  });

  it('dismiss button removes the corresponding toast', async () => {
    useToastStore.getState().push({ kind: 'info', title: 'removable', ttl: 0 });
    render(<MemoryRouter><Notifications /></MemoryRouter>);
    await waitFor(() => {
      expect(screen.getByText('removable')).toBeInTheDocument();
    });
    const dismissBtn = document.querySelector(
      '[aria-label="notifications.toast.dismiss_aria"]',
    ) as HTMLButtonElement;
    expect(dismissBtn).toBeTruthy();
    fireEvent.click(dismissBtn);
    await waitFor(() => {
      expect(screen.queryByText('removable')).not.toBeInTheDocument();
    });
  });
});
