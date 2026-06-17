/**
 * ModelVersionPill (v0.12d, v0.13b).
 *
 * Small badge that shows which model the sidecar is currently
 * scoring with. Used in the ModelLab page and anywhere else
 * that surfaces a predict result.
 *
 *   "logistic-0.1.0"        → fallback (no model promoted)
 *   "logistic-train-441c352b" → last promoted model
 *   "unknown"               → no probe yet
 *
 * v0.13b — the tooltip now also includes the model's brier score
 * (calibration) when available, so the user can see at a glance
 * how trustworthy the current model is.
 */

import { Cpu } from 'lucide-react';

interface ModelVersionPillProps {
  modelVersion: string | null | undefined;
  /** v0.13b — calibration score of the most recent promoted
   * model. Lower is better (0.0 = perfect, 0.25 = random for binary).
   */
  brierScore?: number | null;
  /** Optional: "compact" (default) shows just the version; "verbose"
   * prepends a "scoring with" prefix. */
  variant?: 'compact' | 'verbose';
  className?: string;
}

export function ModelVersionPill({
  modelVersion,
  brierScore,
  variant = 'compact',
  className = '',
}: ModelVersionPillProps) {
  const text = modelVersion && modelVersion.length > 0
    ? modelVersion
    : 'unknown';
  const prefix = variant === 'verbose' ? 'scoring with ' : '';
  // Tooltip: include brier if available
  const brierPart = (brierScore != null && Number.isFinite(brierScore))
    ? `\nBrier: ${brierScore.toFixed(3)} (lower is better)`
    : '';
  const tooltip = `Currently scoring with: ${text}${brierPart}`;
  return (
    <span
      data-testid="model-version-pill"
      data-version={text}
      data-brier={brierScore ?? ''}
      className={
        'inline-flex items-center gap-1 px-2 h-6 rounded-full text-[10px] font-mono ' +
        'border border-border bg-surface text-fg-secondary ' +
        className
      }
      title={tooltip}
    >
      <Cpu className="w-3 h-3" />
      {prefix}
      <span className="font-semibold">{text}</span>
      {brierScore != null && Number.isFinite(brierScore) && (
        <span
          className="ml-1 px-1 rounded text-[9px]"
          style={{ background: 'var(--surface-2)', color: 'var(--muted)' }}
          title={`Brier score of the most recent promote: ${brierScore.toFixed(3)}`}
        >
          B {brierScore.toFixed(3)}
        </span>
      )}
    </span>
  );
}
