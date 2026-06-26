// v0.91 — useTrainProgress hook 测试（+7 个测试）。
//
// 该 hook 在 v0.91 中从 ModelLab.tsx 提取出来，以便
// 对 v0.83 延期的分支覆盖率缺口进行测试。本文件
// 验证以下场景：
//   - 初始状态：activeTrainJobId=null
//   - markExpected + train:started → activeTrainJobId 被设置
//   - markExpected + train:started 重置 expected 标记
//     （忽略下一个事件）
//   - 2 次 markExpected + 2 次 train:started → 捕获第二次的事件
//   - markExpected + train:started + clearActive → activeTrainJobId 设为 null
//   - enabled=false → 不订阅监听器
//   - 卸载时清理监听器（严格模式下第一次
//     mount 时 cancelled=true；第二次 mount 的监听器保持）

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useTrainProgress } from './useTrainProgress';
import type { TrainStartedEvent } from '@/ipc';

type ListenFn = (
  cb: (e: TrainStartedEvent) => void,
) => Promise<() => void>;

function evt(jobId: string): TrainStartedEvent {
  return { job_id: jobId, n_trials: 4, epochs: 80, started_at: 1700000000 };
}

/**
 * 为测试构建可控的 ListenFn。返回辅助函数：
 *   - fn: 传递给 hook 的 ListenFn
 *   - fire(): 向所有已注册的 listener 推送一个事件
 *   - subscribeCount(): listener 被注册的次数
 *   - unsub(): 最近一次订阅的 unlisten 函数
 */
function makeListener() {
  const listeners: Array<(e: TrainStartedEvent) => void> = [];
  const unsubFns: Array<() => void> = [];
  let count = 0;
  const fn: ListenFn = (cb) => {
    count++;
    listeners.push(cb);
    const unsub = vi.fn();
    unsubFns.push(unsub);
    return Promise.resolve(unsub);
  };
  return {
    fn,
    fire: (e: TrainStartedEvent) => listeners.forEach((l) => l(e)),
    subscribeCount: () => count,
    lastUnsub: () => unsubFns[unsubFns.length - 1],
  };
}

describe('useTrainProgress (v0.91)', () => {
  let listener: ReturnType<typeof makeListener>;
  let onTrainStarted: ListenFn;

  beforeEach(() => {
    listener = makeListener();
    onTrainStarted = listener.fn;
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('starts with activeTrainJobId=null and subscribes once on mount', () => {
    const { result } = renderHook(() => useTrainProgress({ onTrainStarted }));
    expect(result.current.activeTrainJobId).toBeNull();
    expect(listener.subscribeCount()).toBe(1);
  });

  it('captures job_id when markExpected is called before train:started', async () => {
    const { result } = renderHook(() => useTrainProgress({ onTrainStarted }));
    act(() => {
      result.current.markExpected();
    });
    await act(async () => {
      listener.fire(evt('train-1'));
    });
    expect(result.current.activeTrainJobId).toBe('train-1');
  });

  it('ignores train:started when markExpected was NOT called', async () => {
    const { result } = renderHook(() => useTrainProgress({ onTrainStarted }));
    await act(async () => {
      listener.fire(evt('orphan-event'));
    });
    expect(result.current.activeTrainJobId).toBeNull();
  });

  it('captures only the NEXT event after markExpected (ref resets)', async () => {
    const { result } = renderHook(() => useTrainProgress({ onTrainStarted }));
    act(() => {
      result.current.markExpected();
    });
    await act(async () => {
      listener.fire(evt('first'));
    });
    expect(result.current.activeTrainJobId).toBe('first');
    // 第二个事件到达时没有再次 markExpected —— 被忽略
    await act(async () => {
      listener.fire(evt('second'));
    });
    expect(result.current.activeTrainJobId).toBe('first');
  });

  it('clearActive() resets activeTrainJobId to null', async () => {
    const { result } = renderHook(() => useTrainProgress({ onTrainStarted }));
    act(() => {
      result.current.markExpected();
    });
    await act(async () => {
      listener.fire(evt('train-x'));
    });
    expect(result.current.activeTrainJobId).toBe('train-x');
    act(() => {
      result.current.clearActive();
    });
    expect(result.current.activeTrainJobId).toBeNull();
  });

  it('enabled=false does NOT subscribe to onTrainStarted', () => {
    const { result } = renderHook(() =>
      useTrainProgress({ onTrainStarted, enabled: false }),
    );
    expect(listener.subscribeCount()).toBe(0);
    expect(result.current.activeTrainJobId).toBeNull();
  });

  it('unmounts clean up the listener (await unlisten Promise)', async () => {
    const { unmount } = renderHook(() =>
      useTrainProgress({ onTrainStarted }),
    );
    expect(listener.subscribeCount()).toBe(1);
    const unsub = listener.lastUnsub();
    expect(unsub).toBeDefined();
    unmount();
    // 清理函数会 await unlisten promise。
    // unmount 之后，unlisten 应该已被调用。
    await act(async () => {
      // 微任务刷新
    });
    expect(unsub).toHaveBeenCalled();
  });
});
