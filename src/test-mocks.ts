// polyrocket —— 共享的测试 mock fixtures（v0.67f）。
//
// 将原本在 6+ 个测试文件中重复的 `@/ipc` mock
// 模式集中起来。v0.66 总结出的硬经验：
//
//   1. 任何 `@/ipc` mock 中都必须包含
//      `sendNotification`。toast-store.ts
//      在系统级启用 toast 时会调用它；
//      没有它时调用会解析为 undefined 并
//      抛出未处理的 reject。
//   2. `requestNotificationPermission` 同理。
//   3. zustand store mock（例如 `usePrefsStore`）
//      需要同时具备 hook 和 `getState()` 方法，
//      因为 toast-store.ts 内部使用
//      `usePrefsStore.getState()`（而非 hook）。
//
// **用法**：
//
//   import { createIpcMock } from '@/test-mocks';
//   vi.mock('@/ipc', () => createIpcMock({
//     llmProviderList: vi.fn().mockResolvedValue([]),
//     // 任何其他需要覆盖的 IPC 方法
//   }));
//
// `createIpcMock(overrides)` 返回一个默认将
// `sendNotification` 和 `requestNotificationPermission`
// 设为 `vi.fn().mockResolvedValue(undefined)`
// 的对象。通过 `overrides` 参数可以覆盖
// 任何方法。任何**未覆盖**的方法都是
// `vi.fn()` 返回 undefined（以便测试可以
// 断言调用，而无需测试文件列举组件
// 可能用到的每一个 IPC）。

import { vi } from 'vitest';

/**
 * 构造一个 `@/ipc` mock 对象，其中包含始终
 * 必需的 `sendNotification` 和
 * `requestNotificationPermission`，以及任何
 * 测试特定的覆盖。
 *
 * 结果适合作为 `vi.mock('@/ipc',
 * () => createIpcMock({ ... }))` factory 的返回值。
 *
 * @param overrides IPC 方法名 → mock 实现的映射。
 *                  这里未列出的任何方法都会设置为
 *                  默认的 `vi.fn()`（返回 undefined），
 *                  以便测试能够
 *                  `expect(mock.foo).toHaveBeenCalled()`
 *                  而无需显式设置返回值。
 *
 * **v0.69a**：将参数/返回值类型从
 * `Record<string, ReturnType<typeof vi.fn>>`（vitest 4.x
 * 更严格的 `Mock<Procedure | Constructable>`）改为
 * `Record<string, (...args: any[]) => any>`。原因：
 * 测试文件经常使用类似
 * `llmAnalyze: () => mockLlmAnalyze()` 的内联
 * 箭头函数，其中 `vi.fn()` 的类型是
 * `Mock<...>`。vitest 4.x 会拒绝将 `() => any`
 * 赋值给 `Mock<...>`。更宽松的签名同时
 * 接受原始函数和 `vi.fn()` 实例。
 */
export function createIpcMock(
  overrides: Record<string, (...args: any[]) => any> = {},
): Record<string, (...args: any[]) => any> {
  return {
    // v0.67f —— 始终启用。toast-store.ts 在渲染
    // 系统级 toast 时会用到这两个。如果
    // 没有这些 mock，import 会解析为
    // undefined，并出现「sendNotification is not a function」
    // 未处理的 reject。
    sendNotification: vi.fn().mockResolvedValue(undefined),
    requestNotificationPermission: vi.fn().mockResolvedValue(true),
    // 默认 stub 覆盖所有其他 IPC。
    // override map 中的键（如果匹配）优先。
    ...overrides,
  };
}

/**
 * 构造一个同时包含 hook 和 `getState()` 方法的
 * `usePrefsStore` mock。需要 zustand 风格的 API
 * 是因为 toast-store.ts 在读取偏好时
 * 直接调用 `usePrefsStore.getState()`
 * （不通过 hook）。
 *
 * @param state 初始状态。默认为最小化的「全部关闭」stub。
 */
export function createPrefsStoreMock(
  state: Record<string, unknown> = {},
): ((selector?: unknown) => unknown) & { getState: () => unknown } {
  const defaults = {
    autoPromoteAfterTrain: false,
    autoPromoteNotify: false,
    notificationsEnabled: true,
    ...state,
  };
  const usePrefsStore: any = () => defaults;
  usePrefsStore.getState = () => defaults;
  return usePrefsStore;
}
