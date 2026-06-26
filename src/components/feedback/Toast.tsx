// polyrocket —— Toast viewport (v0.66b 密度)。
//
// 在固定右下角堆叠中渲染全局 toast 队列
// (info / success / warning / error)。与 `@/stores/toast-store`
// 的 `useToastStore` 配合 —— 通过 `toast.info()` /
// `.success()` / `.warning()` / `.error()` 推入 toast 并在此渲染。
//
// **为什么拆成 ToastViewport + ToastHost 两个**:host 是放在
// App 根处的轻量自动挂载组件(让监听器在 app load 时只订阅
// 一次)。viewport 是真正的 UI。拆开后单元测试可直接渲染
// viewport,不必启动整个 app shell。
//
// **颜色 token** —— `KIND_CLS` 的 4 个条目是 toast 颜色
// 决定的唯一位置。新增 ToastKind 时,需同时在此处和
// `stores/toast-store.ts`(kind union 类型)添加条目。
//
// v0.119 —— 增加进场/离场动画(`animate-toast-in` 160ms)。
// 在 `prefers-reduced-motion` 下,globals.css 中的 @media
// 规则会把 animation-duration 缩短到 0.01ms(几乎瞬时)。

import { useEffect, type ReactNode } from 'react';
import { X } from 'lucide-react';
import { useToastStore, type Toast } from '@/stores/toast-store';
import { cn } from '@/lib/cn';

// v0.66b —— 每种 kind 的颜色 token。4 套主题
// 共用此映射;实际颜色取自当前主题的
// CSS 变量(bg-accent、bg-bull 等)。
const KIND_CLS: Record<Toast['kind'], string> = {
  info: 'border-accent/40 bg-accent/10 text-fg',
  success: 'border-bull/40 bg-bull/10 text-fg',
  warning: 'border-warning/40 bg-warning/10 text-fg',
  error: 'border-bear/40 bg-bear/10 text-fg',
};

/**
 * 将当前 toast 队列以固定右下角堆叠形式渲染。
 * 容器元素为 `aria-live="polite"`,屏幕阅读器
 * 可朗读新 toast(不抢占焦点)。
 *
 * Toast 沿垂直方向(column-flex)堆叠,最新的
 * 在底部(对齐 macOS 通知顺序)。每个 toast
 * 有一个 dismiss 按钮(X),调用
 * `useToastStore.dismiss(id)`。
 */
export function ToastViewport(): ReactNode {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);

  return (
    <div
      className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 w-[340px] pointer-events-none"
      aria-live="polite"
    >
      {toasts.map((t) => (
        <div
          key={t.id}
          data-testid="toast"
          data-toast-kind={t.kind}
          className={cn(
            'pointer-events-auto rounded-md border px-3 py-2 shadow-lg flex items-start gap-2 animate-toast-in',
            KIND_CLS[t.kind],
          )}
        >
          <div className="flex-1 min-w-0">
            <div className="text-[12px] font-medium">{t.title}</div>
            {t.body && (
              <div className="text-[11px] text-fg-secondary mt-0.5 break-words">{t.body}</div>
            )}
          </div>
          <button
            onClick={() => dismiss(t.id)}
            className="text-muted hover:text-fg shrink-0"
            aria-label="dismiss"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
}

/**
 * 在 App 根处只挂载一次。`useEffect` 本身是
 * no-op,只触发 `getState()` 以确保 zustand
 * store 已在 React 树中初始化。
 *
 * `ToastHost` 包装 `ToastViewport`,调用方只需
 * 在根处放一个 `<ToastHost />`。需要直接渲染
 * viewport 的测试,应直接 import `ToastViewport`,
 * 而不是 `ToastHost`。
 */
export function ToastHost() {
  useEffect(() => {
    // 确保 store 已初始化
    useToastStore.getState();
  }, []);
  return <ToastViewport />;
}
