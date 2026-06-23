/**
 * ModelVersionPill (v0.12d, v0.13b, v0.119).
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
 *
 * v0.119 — all user-visible strings go through i18n
 * (`model.scoring_with`, `model.unknown`, `model.tooltip_with`).
 * `data-version="unknown"` (DOM attribute) stays English for
 * machine-readable consistency (test contract).
 */

import { Cpu } from 'lucide-react';
import { useT } from '@/lib/i18n';

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
  const { t } = useT();
  // `data-version` stays as machine-readable English for test contract.
  const dataVersion = modelVersion && modelVersion.length > 0
    ? modelVersion
    : 'unknown';
  // Display text uses i18n — en "unknown", zh "未知".
  const displayText = modelVersion && modelVersion.length > 0
    ? modelVersion
    : t('model.unknown', { default: 'unknown' });
  const prefix = variant === 'verbose'
    ? t('model.scoring_with', { default: 'scoring with' }) + ' '
    : '';
  // Tooltip: include brier if available
  const brierPart = (brierScore != null && Number.isFinite(brierScore))
    ? `\n${t('model.brier_label', { default: 'Brier: {{score}} (lower is better)' }).replace('{{score}}', brierScore.toFixed(3))}`
    : '';
  const tooltip = t('model.tooltip_with', { model: displayText }) + brierPart;
  return (
    <span
      data-testid="model-version-pill"
      data-version={dataVersion}
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
      <span className="font-semibold">{displayText}</span>
      {brierScore != null && Number.isFinite(brierScore) && (
        <span
          className="ml-1 px-1 rounded text-[9px]"
          style={{ background: 'var(--surface-2)', color: 'var(--muted)' }}
          title={t('model.brier_promote_label', {
            default: 'Brier score of the most recent promote: {{score}}',
          }).replace('{{score}}', brierScore.toFixed(3))}
        >
          B {brierScore.toFixed(3)}
        </span>
      )}
    </span>
  );
}
