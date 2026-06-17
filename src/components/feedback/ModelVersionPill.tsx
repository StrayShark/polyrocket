/**
 * ModelVersionPill (v0.12d).
 *
 * Small badge that shows which model the sidecar is currently
 * scoring with. Used in the ModelLab page and anywhere else
 * that surfaces a predict result.
 *
 *   "logistic-0.1.0"        → fallback (no model promoted)
 *   "logistic-train-441c352b" → last promoted model
 *   "unknown"               → no probe yet
 */

import { Cpu } from 'lucide-react';

interface ModelVersionPillProps {
  modelVersion: string | null | undefined;
  /** Optional: "compact" (default) shows just the version; "verbose"
   * prepends a "scoring with" prefix. */
  variant?: 'compact' | 'verbose';
  className?: string;
}

export function ModelVersionPill({
  modelVersion,
  variant = 'compact',
  className = '',
}: ModelVersionPillProps) {
  const text = modelVersion && modelVersion.length > 0
    ? modelVersion
    : 'unknown';
  const prefix = variant === 'verbose' ? 'scoring with ' : '';
  return (
    <span
      data-testid="model-version-pill"
      data-version={text}
      className={
        'inline-flex items-center gap-1 px-2 h-6 rounded-full text-[10px] font-mono ' +
        'border border-border bg-surface text-fg-secondary ' +
        className
      }
      title={`Currently scoring with: ${text}`}
    >
      <Cpu className="w-3 h-3" />
      {prefix}
      <span className="font-semibold">{text}</span>
    </span>
  );
}
