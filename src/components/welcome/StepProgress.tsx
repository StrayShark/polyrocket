// v0.53b —— StepProgress 组件。
//
// 在 Welcome 路由顶部渲染 6 点进度指示。
// 用户推进时点会填充;当前步骤用
// accent 色 ring 高亮。

import type { WelcomeStep } from '@/stores/welcome-store';
import { cn } from '@/lib/cn';

export function StepProgress({
  current,
  steps,
}: {
  current: WelcomeStep;
  steps: WelcomeStep[];
}) {
  const currentIndex = steps.indexOf(current);
  return (
    <div
      className="flex items-center gap-1.5"
      data-testid="welcome-step-progress"
    >
      {steps.map((s, i) => {
        const isPast = i < currentIndex;
        const isCurrent = i === currentIndex;
        return (
          <div
            key={s}
            className={cn(
              'h-1.5 flex-1 rounded-full transition-colors duration-base ease-out-cubic',
              isPast && 'bg-accent',
              isCurrent && 'bg-accent ring-2 ring-accent/30',
              !isPast && !isCurrent && 'bg-surface-2',
            )}
            data-testid={`welcome-step-dot-${s}`}
            data-state={isCurrent ? 'current' : isPast ? 'past' : 'future'}
          />
        );
      })}
    </div>
  );
}
