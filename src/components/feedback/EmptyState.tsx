import type { ReactNode } from 'react';
import { Inbox } from 'lucide-react';

/** `EmptyState` props。
 *   - `icon` — 顶部 icon（默认 Inbox 箱子）
 *   - `title` — 主标题
 *   - `description` — 副标题（可空）
 *   - `action` — 底部 button/link 区域（可空，常见「Create first X」CTA）
 */
export interface EmptyStateProps {
  icon?: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
}

/**
 * `EmptyState` —— 空状态展示组件。
 *
 * **何时用**：list 查询返回 0 条 / object 不存在时，**不**用 ErrorState（不是错误）。
 *
 * **3 主题**：背景 `bg-surface-2` + 文字 `text-fg`/`text-muted` 走 CSS variable，
 * 3 个主题都正确对比。
 */
export function EmptyState({ icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center text-center py-10 px-4">
      <div className="w-10 h-10 rounded-full bg-surface-2 grid place-items-center text-muted mb-3">
        {icon ?? <Inbox className="w-5 h-5" />}
      </div>
      <h3 className="text-[13px] font-medium text-fg">{title}</h3>
      {description && (
        <p className="text-[12px] text-muted mt-1 max-w-sm">{description}</p>
      )}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}
