/**
 * TimelinePill(v0.119 —— Cursor 合并)。
 *
 * LLM 分析生命周期的阶段标记,限定在产品内
 * agent timeline 可视化场景(遵循 Cursor 设计规则:
 * pastel 颜色只用于 timeline UI,绝不用作系统 action 色)。
 *
 * 对应 Cursor 的 5 个 timeline token:
 *   - thinking → 桃色(#dfa88f)
 *   - grep     → 薄荷色(#9fc9a2)
 *   - read     → 蓝色(#9fbbe0)
 *   - edit     → 淡紫色(#c0a8dd)
 *   - done     → gold(#c08532,白色文字)
 *
 * 排版(Cursor `caption-uppercase` 规范 —— 以 Tailwind 工具类
 * 组合实现,以让 twMerge 正常工作):
 *   - 11px(`text-xs`)/ 600(`font-semibold`)/ +0.88px 字距 /
 *     UPPERCASE(`uppercase`)
 *   - 内边距:4px 10px(`px-2.5`)
 *   - 圆角:pill 9999px(`rounded-pill`)
 *
 * 使用场景(v0.119):
 *   - `/analysis` 页面 —— LLM analysis 进度
 *   - `/brief` 页面 —— Today's Edge:"ANALYSIS DONE" 徽章
 *   - `/football/fixtures/:id` —— "MODEL GENERATED" 阶段标记
 */

import { cn } from '@/lib/cn';

export type TimelineStage = 'thinking' | 'grep' | 'read' | 'edit' | 'done';

export interface TimelinePillProps {
  stage: TimelineStage;
  /** 可选的自定义 label(默认从 stage 派生)。 */
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
  // `done` 使用 bg 文字(白字配 gold,最接近 Cursor 的 `on-primary` 规则)。
  done: 'bg-timeline-done text-bg',
};

export function TimelinePill({ stage, label, className }: TimelinePillProps) {
  const text = label ?? STAGE_LABEL[stage];
  return (
    <span
      data-testid="timeline-pill"
      data-stage={stage}
      className={cn(
        // text-xs = 11px, font-semibold = 600, tracking-caption-uppercase = 0.88px (Tailwind 值注释)
        'inline-flex items-center h-5 px-2.5 rounded-pill text-xs font-semibold tracking-caption-uppercase uppercase whitespace-nowrap border border-transparent',
        STAGE_CLASSES[stage],
        className,
      )}
    >
      {text}
    </span>
  );
}
