// v0.53b — StepProgress component.
//
// Renders the 6-dot progress indicator at the top
// of the Welcome route. Dots are filled in as the
// user advances; the current step is highlighted
// with an accent-colored ring.

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
