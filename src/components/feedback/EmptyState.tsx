import type { ReactNode } from 'react';
import { Inbox } from 'lucide-react';

export interface EmptyStateProps {
  icon?: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
}

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
