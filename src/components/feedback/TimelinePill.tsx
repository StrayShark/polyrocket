/**
 * TimelinePill (v0.119 — Cursor-merge).
 *
 * Stage marker for LLM analysis lifecycle, scoped to in-product agent
 * timeline visualizations (per Cursor design rule: pastels only on timeline UI,
 * never as system action colors).
 *
 * Maps to Cursor's 5 timeline tokens:
 *   - thinking → peach  (#dfa88f)
 *   - grep     → mint   (#9fc9a2)
 *   - read     → blue   (#9fbbe0)
 *   - edit     → lavender (#c0a8dd)
 *   - done     → gold   (#c08532, white text)
 *
 * Style (Cursor `caption-uppercase` spec — implemented as Tailwind utility
 * composition to keep twMerge happy):
 *   - 11px (`text-xs`) / 600 (`font-semibold`) / +0.88px tracking /
 *     UPPERCASE (`uppercase`)
 *   - padding: 4px 10px (`px-2.5`)
 *   - rounded: pill 9999px (`rounded-pill`)
 *
 * Use cases (v0.119):
 *   - `/analysis` page — LLM analysis progress
 *   - `/brief` page — Today's Edge: "ANALYSIS DONE" pill
 *   - `/football/fixtures/:id` — "MODEL GENERATED" stage marker
 */

import { cn } from '@/lib/cn';

export type TimelineStage = 'thinking' | 'grep' | 'read' | 'edit' | 'done';

export interface TimelinePillProps {
  stage: TimelineStage;
  /** Optional override label (otherwise derived from stage). */
  label?: string;
  className?: string;
}

const STAGE_LABEL: Record<TimelineStage, string> = {
  thinking: 'Thinking',
  grep: 'Grepping',
  read: 'Reading',
  edit: 'Editing',
  done: 'Done',
};

const STAGE_CLASSES: Record<TimelineStage, string> = {
  thinking: 'bg-timeline-thinking text-fg',
  grep: 'bg-timeline-grep text-fg',
  read: 'bg-timeline-read text-fg',
  edit: 'bg-timeline-edit text-fg',
  // `done` uses bg text (white text on gold, closest to Cursor's `on-primary` rule).
  done: 'bg-timeline-done text-bg',
};

export function TimelinePill({ stage, label, className }: TimelinePillProps) {
  const text = label ?? STAGE_LABEL[stage];
  return (
    <span
      data-testid="timeline-pill"
      data-stage={stage}
      className={cn(
        // text-xs = 11px, font-semibold = 600, tracking-caption-uppercase = 0.88px
        'inline-flex items-center h-5 px-2.5 rounded-pill text-xs font-semibold tracking-caption-uppercase uppercase whitespace-nowrap border border-transparent',
        STAGE_CLASSES[stage],
        className,
      )}
    >
      {text}
    </span>
  );
}
