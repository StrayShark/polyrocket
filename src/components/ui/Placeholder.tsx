import type { ReactNode } from 'react';

/** `Placeholder` props。
 *   - `title` — 页面大标题
 *   - `icon` — 中部 icon（视觉锚点）
 *   - `hint` — 副标题（说明这是占位页）
 *   - `filename` — 可选，底部显示「Edit {filename}」提示
 */
interface PlaceholderProps {
  title: string;
  icon: ReactNode;
  hint: string;
  filename?: string;
}

/**
 * `Placeholder` —— 路由占位页（未实现的 v0.x 路由用）。
 *
 * **何时用**：spec 里规划但还没实装的路由用这个组件占位，让 nav 能跳转
 * 而不显示「404」。L1 路径都先注册到 `<Routes>`，component = `Placeholder`。
 *
 * **不是 EmptyState**：Placeholder 是「整页占位」，EmptyState 是「列表内
 * 嵌一段空状态」。
 */
export function Placeholder({ title, icon, hint, filename }: PlaceholderProps) {
  return (
    <div className="px-6 py-10">
      <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
      <p className="text-xs mt-0.5" style={{ color: 'var(--muted)' }}>
        {hint}
      </p>
      <div
        className="mt-8 rounded-lg border border-dashed grid place-items-center text-center p-10"
        style={{ borderColor: 'var(--border)', color: 'var(--muted)' }}
      >
        <div style={{ color: 'var(--muted)' }}>{icon}</div>
        <div className="mt-3 text-sm">This route is a placeholder</div>
        {filename && (
          <div className="text-[11px] mt-1" style={{ color: 'var(--muted)' }}>
            Edit <code className="font-mono">{filename}</code>
          </div>
        )}
      </div>
    </div>
  );
}