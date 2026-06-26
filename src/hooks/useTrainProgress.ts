import { useEffect, useRef, useState } from 'react';
import type { TrainStartedEvent } from '@/ipc';

/**
 * Tauri 风格事件监听器订阅的 hook 签名。
 * 返回一个 resolve 为 unlisten 函数的 Promise。
 */
type ListenFn = (
  cb: (e: TrainStartedEvent) => void,
) => Promise<() => void>;

/**
 * `useTrainProgress` —— 封装 `/model-lab` 的训练任务
 * 进度追踪状态机（v0.91）。
 *
 * **状态机**：
 *   - `idle` —— 没有进行中的训练，不期望事件
 *   - `expected` —— 调用方调用了 `markExpected()`（通常
 *     在 `trainMut.mutate()` 之前）。下一个 `train:started`
 *     事件将被捕获。
 *   - `running` —— 已捕获 `train:started` 事件；
 *     `activeTrainJobId` 已被设置。自动重置为 `idle`。
 *
 * **为什么使用自定义 hook**（v0.83 延期）：
 *   ModelLab.tsx 中原本内联的 `useEffect` 难以
 *   测试，因为它既依赖 Tauri 的 `safeListen`
 *   返回 `Promise<UnlistenFn>`，又依赖 React 18 严格
 *   模式下的双 mount 行为（第一次 mount 的监听器
 *   带 `cancelled = true`）。将其提取到 hook 中可以：
 *     1. 在测试中直接传入 `ListenFn`（无需 Tauri 运行时）
 *     2. 确定性测试状态转移
 *     3. 在任何希望监听训练进度的页面中复用
 *        （例如未来的 `/dashboard` 小组件）
 *
 * **调用模式**：
 *   ```ts
 *   const { activeTrainJobId, markExpected, clearActive } = useTrainProgress({
 *     onTrainStarted,
 *   });
 *
 *   const handleClick = () => {
 *     markExpected();
 *     trainMut.mutate();
 *   };
 *   ```
 */
export interface UseTrainProgressResult {
  /** 进行中训练的 job ID，idle 时为 null。 */
  activeTrainJobId: string | null;
  /**
   * 标记下一个 `train:started` 事件应当
   * 被捕获。请在调用 `trainMut.mutate()`
   * 之前调用。v0.91 —— 取代直接修改 ref
   * 的 `expectedTrainRef.current = true`。
   */
  markExpected: () => void;
  /**
   * 手动清空活动 job ID（例如取消按钮
   * 或处理过期事件之后）。v0.91 —— 取代取消处理
   * 函数中的直接 `setActiveTrainJobId(null)`。
   */
  clearActive: () => void;
}

/**
 * 订阅 `train:started` 事件，并在期望
 * （即调用了 `markExpected()` 之后）时捕获 job ID。
 *
 * @param opts.onTrainStarted —— Tauri 事件订阅
 *                              函数（例如 `onTrainStarted`）。
 * @param opts.enabled —— 设为 false 以解绑监听器
 *                        （默认 true）。供测试用来
 *                        验证清理路径。
 */
export function useTrainProgress(opts: {
  onTrainStarted: ListenFn;
  enabled?: boolean;
}): UseTrainProgressResult {
  const { onTrainStarted, enabled = true } = opts;
  const [activeTrainJobId, setActiveTrainJobId] = useState<string | null>(null);
  // 用 ref 避免 listener 闭包依赖
  // markExpected() 的调用位置 —— listener 始终
  // 读取最新值。
  const expectedTrainRef = useRef<boolean>(false);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    const unsubPromise = onTrainStarted((e) => {
      if (cancelled) return;
      if (expectedTrainRef.current) {
        setActiveTrainJobId(e.job_id);
        expectedTrainRef.current = false;
      }
    });
    return () => {
      // Strict-mode 下的双 mount：第一次 mount 的 listener
      // 拥有 cancelled=true；我们仍然 await
      // unlisten Promise 以干净地解绑。
      // .catch 静默，因为测试可能不 await unlisten。
      cancelled = true;
      unsubPromise.then((u) => u()).catch(() => { /* 忽略 */ });
    };
  }, [onTrainStarted, enabled]);

  const markExpected = () => {
    expectedTrainRef.current = true;
  };
  const clearActive = () => {
    setActiveTrainJobId(null);
  };

  return { activeTrainJobId, markExpected, clearActive };
}
