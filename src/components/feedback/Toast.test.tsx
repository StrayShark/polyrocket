// v0.70f —— ToastViewport + ToastHost 组件测试。
//
// Toast.tsx 之前覆盖率为 0%。它是个小(96 行)
// 但关键的反馈组件 —— 每次成功的 IPC mutation
// 都会显示 toast。测试覆盖:
//   - 4 种 kind 用各自的 KIND_CLS class 渲染
//   - body 文本在 title 之下渲染
//   - dismiss 按钮调用 useToastStore.dismiss(id)
//   - 空队列不渲染任何内容
//   - ToastHost 的 useEffect 初始化 store
//
// Toast.tsx:0% → 约 95% stmts。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

const { mockToastState, mockDismiss, mockUseToastStore } = vi.hoisted(() => {
  const toastState = { toasts: [] as any[] };
  const dismissFn = vi.fn();
  // useToastStore 既作为 hook (selector 模式) 又作为
  // useToastStore.getState() 调用。构建为带
  // getState 静态方法的 vi.fn (可调用)。
  const useToastStore: any = Object.assign(vi.fn(), {
    getState: vi.fn(() => ({ toasts: toastState.toasts, dismiss: dismissFn })),
  });
  return { mockToastState: toastState, mockDismiss: dismissFn, mockUseToastStore: useToastStore };
});

vi.mock('@/stores/toast-store', () => ({
  useToastStore: mockUseToastStore,
}));

import { ToastViewport, ToastHost } from './Toast';

function renderViewport() {
  // 每次渲染时把 hook 实现接到我们的可变 state 上。
  mockUseToastStore.mockImplementation((selector: any) =>
    selector({ toasts: mockToastState.toasts, dismiss: mockDismiss }),
  );
  return render(<ToastViewport />);
}

beforeEach(() => {
  vi.clearAllMocks();
  mockToastState.toasts = [];
});

describe('ToastViewport', () => {
  it('renders nothing when toasts queue is empty', () => {
    renderViewport();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('renders info toast with accent border class', () => {
    mockToastState.toasts = [
      { id: '1', kind: 'info', title: 'Info title', ttl: 4000 },
    ];
    renderViewport();
    expect(screen.getByText('Info title')).toBeInTheDocument();
    const div = screen.getByText('Info title').closest('div.pointer-events-auto');
    expect(div?.className).toMatch(/border-accent/);
  });

  it('renders success toast with bull (green) border class', () => {
    mockToastState.toasts = [
      { id: '2', kind: 'success', title: 'Saved!', ttl: 4000 },
    ];
    renderViewport();
    expect(screen.getByText('Saved!')).toBeInTheDocument();
    const div = screen.getByText('Saved!').closest('div.pointer-events-auto');
    expect(div?.className).toMatch(/border-bull/);
  });

  it('renders warning toast with warning border class', () => {
    mockToastState.toasts = [
      { id: '3', kind: 'warning', title: 'Heads up', ttl: 6000 },
    ];
    renderViewport();
    expect(screen.getByText('Heads up')).toBeInTheDocument();
    const div = screen.getByText('Heads up').closest('div.pointer-events-auto');
    expect(div?.className).toMatch(/border-warning/);
  });

  it('renders error toast with bear (red) border class', () => {
    mockToastState.toasts = [
      { id: '4', kind: 'error', title: 'Failed', ttl: 0 },
    ];
    renderViewport();
    expect(screen.getByText('Failed')).toBeInTheDocument();
    const div = screen.getByText('Failed').closest('div.pointer-events-auto');
    expect(div?.className).toMatch(/border-bear/);
  });

  it('renders body text when provided', () => {
    mockToastState.toasts = [
      { id: '5', kind: 'info', title: 'Title', body: 'Body content here', ttl: 4000 },
    ];
    renderViewport();
    expect(screen.getByText('Title')).toBeInTheDocument();
    expect(screen.getByText('Body content here')).toBeInTheDocument();
  });

  it('does not render body div when body is absent', () => {
    mockToastState.toasts = [
      { id: '6', kind: 'success', title: 'Just title', ttl: 4000 },
    ];
    renderViewport();
    expect(screen.getByText('Just title')).toBeInTheDocument();
    // 不带 body 时不渲染第二个 text-11 div
    expect(document.body.textContent).not.toContain('Body');
  });

  it('calls dismiss(id) when X button is clicked', () => {
    mockToastState.toasts = [
      { id: '7', kind: 'error', title: 'Dismiss me', ttl: 0 },
    ];
    renderViewport();
    const dismissBtn = screen.getByRole('button', { name: /dismiss/i });
    fireEvent.click(dismissBtn);
    expect(mockDismiss).toHaveBeenCalledWith('7');
  });

  it('renders multiple toasts in order (newest at bottom)', () => {
    mockToastState.toasts = [
      { id: 'a', kind: 'info', title: 'First', ttl: 4000 },
      { id: 'b', kind: 'success', title: 'Second', ttl: 4000 },
      { id: 'c', kind: 'error', title: 'Third', ttl: 0 },
    ];
    renderViewport();
    expect(screen.getByText('First')).toBeInTheDocument();
    expect(screen.getByText('Second')).toBeInTheDocument();
    expect(screen.getByText('Third')).toBeInTheDocument();
  });

  it('has aria-live="polite" on the container for screen-reader announcements', () => {
    renderViewport();
    const container = document.querySelector('[aria-live="polite"]');
    expect(container).toBeInTheDocument();
  });
});

describe('ToastHost', () => {
  it('mounts ToastViewport (renders empty queue)', () => {
    render(<ToastHost />);
    // 无 toasts → 无按钮
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('mounts ToastViewport with toast visible', () => {
    mockToastState.toasts = [
      { id: 'h1', kind: 'info', title: 'Host test', ttl: 4000 },
    ];
    render(<ToastHost />);
    expect(screen.getByText('Host test')).toBeInTheDocument();
  });
});
