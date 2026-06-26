/**
 * ModelVersionPill(v0.12d、v0.13b、v0.119)。
 *
 * 小徽章,显示 sidecar 当前使用的评分 model。
 * 用于 ModelLab 页面以及其他展示 predict
 * 结果的位置。
 *
 *   "logistic-0.1.0"          → fallback(没有 promoted 的 model)
 *   "logistic-train-441c352b" → 最近一次 promoted 的 model
 *   "unknown"                 → 尚未 probe
 *
 * v0.13b —— tooltip 还会附带 model 的 brier
 * 分数(校准度)(如有),用户一眼就能看出当前
 * model 的可信度。
 *
 * v0.119 —— 所有面向用户的字符串都走 i18n
 * (`model.scoring_with`、`model.unknown`、`model.tooltip_with`)。
 * `data-version="unknown"`(DOM 属性)仍为英文,
 * 以保持机器可读的一致性(test contract)。
 */

import { Cpu } from 'lucide-react';
import { useT } from '@/lib/i18n';

interface ModelVersionPillProps {
  modelVersion: string | null | undefined;
  /** v0.13b —— 最近 promoted 的 model 校准分数。
   * 越低越好(0.0 = 完美,0.25 = 二分类的随机水平)。
   */
  brierScore?: number | null;
  /** 可选:"compact"(默认)只显示版本号;
   * "verbose"在前面加 "scoring with" 前缀。 */
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
  // `data-version` 保持机器可读的英文,以便 test contract 一致。
  const dataVersion = modelVersion && modelVersion.length > 0
    ? modelVersion
    : 'unknown';
  // 显示文本走 i18n —— en 为 "unknown",zh 为 "未知"。
  const displayText = modelVersion && modelVersion.length > 0
    ? modelVersion
    : t('model.unknown', { default: 'unknown' });
  const prefix = variant === 'verbose'
    ? t('model.scoring_with', { default: 'scoring with' }) + ' '
    : '';
  // Tooltip:如有 brier 则附带
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
