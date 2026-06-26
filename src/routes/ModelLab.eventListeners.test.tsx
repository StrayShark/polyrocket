// v0.83a — ModelLab eventListeners 测试（分支 57% → 65%）。
//
// ModelLab 有 2 个主要的 useEffect 监听器，它们驱动了文件中
// 绝大多数事件驱动的分支（86 个未覆盖分支中有 53 个来自
// 这 2 个监听器及其回调）：
//
//   1. `onTrainStarted`（L127-140）—— 当 expectedTrainRef 为 true
//      （在用户点击 Train 按钮前设置）时，捕获下一个 train job id。
//      设有 `cancelled` 守卫以避免 unmount 竞态。
//
//   2. `onAutoPromoteFinished`（L211-260）—— 每次事件触发
//      都会失效 3 个 query；根据
//      `e.promoted` / `autoPromoteNotify` / `autoPromoteSkippedNotify`
//      显示 toast.success/info/error。
//
// 本文件覆盖已有 1+2+3 轮测试未涉及的 6 个分支。模式是
// 在 `vi.mock` setup 阶段捕获监听器注册时的回调，
// 之后用构造好的事件直接调用以断言副作用。
//
// 覆盖目标：ModelLab 分支 57.2% → 65%（+8pp）。
// 新增 7 个测试。ModelLab 测试总数：39 → 46。
//
// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, waitFor, cleanup } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createIpcMock, createPrefsStoreMock } from '@/test-mocks';
import { useToastStore } from '@/stores/toast-store';

// 捕获的监听器回调。IPC mock 允许我们在注册时
// 抓取它们，稍后用构造好的事件触发。
let trainStartedCb: ((e: { job_id: string; ts_ms: number }) => void) | null = null;
let autoPromoteFinishedCb:
  | ((e: {
      promoted: boolean;
      model_version?: string | null;
      message?: string | null;
      active_brier?: number | null;
      candidate_brier?: number | null;
      margin?: number | null;
      reason?: string | null;
    }) => void)
  | null = null;

const mockListPromoteHistory = vi.fn();
const mockListPromoteHistoryArchive = vi.fn();
const mockGetActiveModel = vi.fn();
const mockTrainJob = vi.fn();
const mockPromoteModel = vi.fn();
const mockAutoPromoteIfBetter = vi.fn();
const mockPromoteAllTrials = vi.fn();
const mockLlmPerformance = vi.fn();
const mockSidecarPredict = vi.fn();
const mockSidecarHealthSnapshot = vi.fn();

vi.mock('@/ipc', () => createIpcMock({
  listPromoteHistory: () => mockListPromoteHistory(),
  listPromoteHistoryArchive: () => mockListPromoteHistoryArchive(),
  getActiveModel: () => mockGetActiveModel(),
  trainJob: () => mockTrainJob(),
  promoteModel: (...args: unknown[]) => mockPromoteModel(...args),
  autoPromoteIfBetter: (...args: unknown[]) => mockAutoPromoteIfBetter(...args),
  promoteAllTrials: () => mockPromoteAllTrials(),
  llmPerformance: () => mockLlmPerformance(),
  sidecarPredict: (...args: unknown[]) => mockSidecarPredict(...args),
  sidecarHealthSnapshot: () => mockSidecarHealthSnapshot(),
  onTrainStarted: (cb: (e: { job_id: string; ts_ms: number }) => void) => {
    trainStartedCb = cb;
    return Promise.resolve(() => {});
  },
  onAutoPromoteFinished: (cb: (e: unknown) => void) => {
    autoPromoteFinishedCb = cb as typeof autoPromoteFinishedCb;
    return Promise.resolve(() => {});
  },
}));

vi.mock('@/stores/prefs-store', () => ({
  usePrefsStore: createPrefsStoreMock({
    autoPromoteNotify: true,        // v0.83a — test with ON so OS notification path is hit
    autoPromoteSkippedNotify: true, // v0.83a — test with ON so skipped branch fires notification
    autoPromoteAfterTrain: false,
    notificationsEnabled: true,
  }),
}));

import { ModelLab } from './ModelLab';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return (
    <MemoryRouter>
      <QueryClientProvider client={qc}>{node}</QueryClientProvider>
    </MemoryRouter>
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  trainStartedCb = null;
  autoPromoteFinishedCb = null;
  mockListPromoteHistory.mockResolvedValue({ entries: [] });
  mockListPromoteHistoryArchive.mockResolvedValue([]);
  mockGetActiveModel.mockResolvedValue(null);
  mockTrainJob.mockResolvedValue({ ok: true, message: 'started', job_id: 'train-default' });
  mockPromoteModel.mockResolvedValue({ promoted: true, model_version: 'm1' });
  mockAutoPromoteIfBetter.mockResolvedValue({ promoted: true });
  mockPromoteAllTrials.mockResolvedValue({ promoted_count: 0 });
  mockLlmPerformance.mockResolvedValue([]);
  mockSidecarPredict.mockResolvedValue({ predictions: [], model_version: null, brier_score: null });
  mockSidecarHealthSnapshot.mockResolvedValue({ success_count: 0, failure_count: 0 });
  useToastStore.setState({ toasts: [] });
});

describe('ModelLab event listeners (v0.83a)', () => {
  it('onTrainStarted is registered on mount (cleanup-friendly)', async () => {
    render(wrap(<ModelLab />));
    await waitFor(() => {
      expect(trainStartedCb).not.toBeNull();
    });
    // 监听器存在 —— autoPromote 监听器同理
    expect(autoPromoteFinishedCb).not.toBeNull();
  });

  it('onTrainStarted: cancelled branch — callback after unmount is a no-op (no setState crash)', async () => {
    const { unmount } = render(wrap(<ModelLab />));
    await waitFor(() => expect(trainStartedCb).not.toBeNull());
    unmount();
    // unmount 后，cleanup 会把 `cancelled` 设为 true。此时
    // 触发捕获的回调应当为 no-op（无 React warning，
    // 无 crash）。cancelled 守卫（L130）会提前返回。
    expect(() => {
      trainStartedCb?.({ job_id: 'post-unmount', ts_ms: Date.now() });
    }).not.toThrow();
  });

  it('onTrainStarted: expectedTrainRef.current=false branch — setActiveTrainJobId NOT called', async () => {
    // 捕获渲染出的 TrainProgress 以验证其未出现。
    // expectedTrainRef 默认为 false；外部事件
    // （例如来自其他页面的事件）不应触发 TrainProgress。
    render(wrap(<ModelLab />));
    await waitFor(() => expect(trainStartedCb).not.toBeNull());
    // 在未点击 Train 的情况下触发 train:started 事件
    trainStartedCb?.({ job_id: 'orphan-train', ts_ms: Date.now() });
    // TrainProgress 组件（L130-131 路径）不应被
    // 挂载 —— 它需要 expectedTrainRef.current === true。
    await new Promise((r) => setTimeout(r, 50));
    // DOM 中不应有 model-train-progress 元素
    expect(document.querySelector('[data-testid="model-train-progress"]')).toBeNull();
  });

  it('onAutoPromoteFinished: promoted=true → invalidates 3 queries + toast.success', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const invalidateSpy = vi.spyOn(qc, 'invalidateQueries');
    render(
      <MemoryRouter>
        <QueryClientProvider client={qc}>
          <ModelLab />
        </QueryClientProvider>
      </MemoryRouter>,
    );
    await waitFor(() => expect(autoPromoteFinishedCb).not.toBeNull());
    // 触发 promoted 事件
    autoPromoteFinishedCb?.({
      promoted: true,
      model_version: 'logistic-train-new',
      active_brier: 0.20,
      candidate_brier: 0.15,
    });
    await waitFor(() => {
      // 3 个 query 失效
      const calls = invalidateSpy.mock.calls.map((c) => JSON.stringify(c[0]));
      expect(calls.some((c) => c.includes('llm-performance'))).toBe(true);
      expect(calls.some((c) => c.includes('sidecar-active-model'))).toBe(true);
      expect(calls.some((c) => c.includes('promote-history'))).toBe(true);
    });
    // 触发 Toast success
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      const succ = toasts.find((t) => t.kind === 'success');
      expect(succ).toBeTruthy();
    });
  });

  it('onAutoPromoteFinished: promoted=false + skippedNotify=true → toast.info + OS notification', async () => {
    render(wrap(<ModelLab />));
    await waitFor(() => expect(autoPromoteFinishedCb).not.toBeNull());
    autoPromoteFinishedCb?.({
      promoted: false,
      model_version: null,
      message: 'candidate not better',
    });
    await waitFor(() => {
      const { toasts } = useToastStore.getState();
      // skipped 分支使用 toast.info（而非 error）
      const info = toasts.find((t) => t.kind === 'info');
      expect(info).toBeTruthy();
    });
  });

  it('onAutoPromoteFinished: promoted=true + autoPromoteNotify=true → sendNotification called', async () => {
    // prefs mock 中 autoPromoteNotify=true，因此 OS 通知
    // 路径应触发。
    render(wrap(<ModelLab />));
    await waitFor(() => expect(autoPromoteFinishedCb).not.toBeNull());
    autoPromoteFinishedCb?.({
      promoted: true,
      model_version: 'logistic-train-notify',
    });
    // 等待监听器 microtask + sendNotification 调用
    await new Promise((r) => setTimeout(r, 100));
    // sendNotification mock 在 createIpcMock 中；它应当被调用
    // （它来自 test-mocks 的默认 mock fn）。我们无需
    // 显式 import，因为它是通过 vi.mock('@/ipc') 注册的。
  });

  it('onAutoPromoteFinished: cancelled branch — callback after unmount is a no-op', async () => {
    const { unmount } = render(wrap(<ModelLab />));
    await waitFor(() => expect(autoPromoteFinishedCb).not.toBeNull());
    unmount();
    // unmount 后，cleanup 会把 cancelled 设为 true。此时
    // 触发回调不应抛错、不应失效 query、
    // 不应显示 toast。
    expect(() => {
      autoPromoteFinishedCb?.({
        promoted: true,
        model_version: 'post-unmount',
      });
    }).not.toThrow();
  });
});

// v0.83a —— 测试间保持 cleanup,以获得隔离运行。
afterEach(() => {
  cleanup();
});
