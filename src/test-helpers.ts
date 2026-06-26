// polyrocket —— 测试辅助工具（v0.66d）。
//
// 一个跨 vitest 测试文件共享的小型工具模块。
// 目前导出：
//
//   `withFakeTimersAndState(fn)`
//     包装一个需要 vi.useFakeTimers +
//     React state 更新的异步测试体。在 happy-dom +
//     React 18 中，来自 setTimeout 回调的
//     state 更新在微任务 flush 之前
//     不会传播到测试 rig。
//     此辅助函数推进假定时器，让出
//     微任务队列，然后重新断言。
//
//     用法：
//       it('foo', async () => {
//         await withFakeTimersAndState(async () => {
//           rig.fireKey('g');
//           vi.advanceTimersByTime(1500);
//         });
//         expect(rig.pendingPrefix).toBeNull();
//       });
//
//   存在原因（v0.65b ship log）：原始的
//   keyboard-nav prefix-timeout 测试无法让
//   来自 setTimeout 回调的 React setState
//   到达测试 rig。尝试了 4 种变通方法：
//
//     1. vi.advanceTimersByTime(1500) in act()  — 仍为 'g'
//     2. vi.runAllTimers() in act()             — 仍为 'g'
//     3. vi.useFakeTimers({ toFake: [...] })    — 仍为 'g'
//     4. await act(async () => { advance; await Promise.resolve() })
//                                               — 仍为 'g'
//
//   在 happy-dom 中有效的模式：在
//   advance 之后（同步触发回调）通过
//   `await Promise.resolve()` 让出真实微任务。
//   此辅助函数封装了这一过程。
//   v0.66d 使用此辅助重新尝试了
//   keyboard-nav 测试，prefix-timeout
//   测试现已通过。

import { act } from '@testing-library/react';

export async function withFakeTimersAndState<T>(
  fn: () => T | Promise<T>,
): Promise<T> {
  return act(async () => {
    const result = await fn();
    // 让出微任务队列，使 React 的调度器
    // 能够 flush 来自定时器回调内部的
    // 任何待处理 setState。如果没有这一步，
    // 由 setTimeout 调度的 state 更新会
    // 卡在 React 批处理中，无法到达测试 rig。
    await Promise.resolve();
    // 再让出一次以应对嵌套的调度器 pass。
    // （React 18 有时需要 2 次微任务 flush
    // 才能将来自 setTimeout 回调的
    // useState 更新完全传播到测试断言中。）
    await Promise.resolve();
    return result;
  });
}
