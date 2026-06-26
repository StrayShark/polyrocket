/**
 * ScoreMatrix —— 以热力图方式可视化 5×5 Poisson 比分概率矩阵。
 *
 * 每个单元格显示 P(主队=i, 客队=j),由 Dixon-Coles 调整的
 * Poisson 模型计算得出。颜色强度随概率缩放。矩阵下方
 * 列出最可能的前 3 个比分。仅在 MarketDetail 中渲染,
 * 用于 football 市场。
 *
 * @param marketId - 要计算矩阵的 Polymarket 市场 ID。
 */
import { useQuery } from '@tanstack/react-query';
import { poissonScoreMatrix } from '@/ipc';
import { Card } from '@/components/base/Card';
import { Skeleton } from '@/components/feedback/Skeleton';
import { ErrorState } from '@/components/feedback/ErrorState';
import { useT } from '@/lib/i18n';

interface ScoreMatrixProps {
  marketId: string;
}

/** 根据概率(0-1)返回从透明到 accent 渐变的颜色。 */
function heatColor(prob: number): string {
  const alpha = Math.min(prob / 0.3, 1); // 在 30% 处饱和
  return `rgba(46, 125, 50, ${alpha * 0.8 + 0.05})`;
}

export function ScoreMatrix({ marketId }: ScoreMatrixProps) {
  const { t } = useT();
  const { data, isLoading, error } = useQuery({
    queryKey: ['poisson-matrix', marketId],
    queryFn: () => poissonScoreMatrix(marketId),
    enabled: !!marketId,
    staleTime: 5 * 60_000,
  });

  if (isLoading) return <Skeleton className="h-48" />;
  if (error) return <ErrorState message={String(error)} />;
  if (!data) return null;

  const { lambda_h, lambda_a, matrix, most_likely } = data;
  const labels = ['0', '1', '2', '3', '4'];

  // 防止 lambda 值为 undefined(IPC 可能返回 null)。
  const lh = lambda_h ?? 0;
  const la = lambda_a ?? 0;

  return (
    <Card title={t('poisson.score_matrix')} description={`λ ${t('poisson.lambda_h')}=${lh.toFixed(2)}, λ ${t('poisson.lambda_a')}=${la.toFixed(2)} (Dixon-Coles adjusted)`}>
      <div className="overflow-x-auto">
        <table className="text-[10px] font-mono">
          <thead>
            <tr>
              <th className="p-1" style={{ color: 'var(--muted)' }}>H\A</th>
              {labels.map((l) => (
                <th key={l} className="p-1 text-center" style={{ color: 'var(--muted)' }}>{l}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {matrix.map((row, i) => (
              <tr key={i}>
                <td className="p-1 font-bold text-center" style={{ color: 'var(--muted)' }}>{labels[i]}</td>
                {row.map((prob, j) => (
                  <td
                    key={j}
                    className="p-1 text-center rounded"
                    style={{
                      background: heatColor(prob),
                      color: prob > 0.15 ? 'white' : 'var(--fg)',
                      minWidth: '48px',
                    }}
                    title={`P(${i}-${j}) = ${(prob * 100).toFixed(1)}%`}
                  >
                    {(prob * 100).toFixed(1)}%
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {most_likely.length > 0 && (
        <div className="mt-3 pt-3 border-t" style={{ borderColor: 'var(--border)' }}>
          <p className="text-[11px] mb-1" style={{ color: 'var(--muted)' }}>{t('poisson.most_likely')}:</p>
          <div className="flex gap-2">
            {most_likely.slice(0, 3).map(([score, prob], i) => (
              <span
                key={i}
                className="px-2 py-1 rounded text-[11px] font-mono"
                style={{ background: 'var(--surface-2)', color: 'var(--fg)' }}
              >
                {score} · {(prob * 100).toFixed(1)}%
              </span>
            ))}
          </div>
        </div>
      )}
    </Card>
  );
}
