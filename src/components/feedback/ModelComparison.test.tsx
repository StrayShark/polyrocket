/**
 * v0.40b —— ModelComparison 组件测试。
 *
 * 多 model 对比 modal 并排展示 2-3 个 model
 * 版本。"Brier 最低" 的 entry 会被高亮为获胜者。
 */

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, cleanup } from '@testing-library/react';
import { ModelComparison } from '@/components/feedback/ModelComparison';
import type { PromoteHistoryEntry } from '@/ipc';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k, locale: 'en' as const }),
}));

const E1: PromoteHistoryEntry = {
  job_id: 'a',
  model_version: 'logistic-a',
  promoted_at_ms: 1_700_000_000_000,
  best_brier: 0.20,
  best_params: { lr: 0.01, reg: 0.001 },
  trial_index: null,
};
const E2: PromoteHistoryEntry = {
  job_id: 'b',
  model_version: 'logistic-b-t0',
  promoted_at_ms: 1_700_001_000_000,
  best_brier: 0.18,
  best_params: { lr: 0.01, reg: 0.001 },
  trial_index: 0,
};
const E3: PromoteHistoryEntry = {
  job_id: 'c',
  model_version: 'logistic-c-t2',
  promoted_at_ms: 1_700_002_000_000,
  best_brier: 0.16,
  best_params: { lr: 0.005, reg: 0.0001 },
  trial_index: 2,
};

describe('ModelComparison (v0.40b)', () => {
  beforeEach(() => {
    // 空操作;仅为与其他测试对称
  });
  afterEach(() => {
    cleanup();
  });

  it('renders 2 columns when given 2 entries', () => {
    render(<ModelComparison open onClose={() => {}} entries={[E1, E2]} />);
    const cols = screen.getAllByTestId('model-comparison-col');
    expect(cols).toHaveLength(2);
    expect(screen.getByTestId('model-comparison')).toHaveAttribute('data-count', '2');
  });

  it('renders 3 columns when given 3 entries', () => {
    render(<ModelComparison open onClose={() => {}} entries={[E1, E2, E3]} />);
    const cols = screen.getAllByTestId('model-comparison-col');
    expect(cols).toHaveLength(3);
    expect(screen.getByTestId('model-comparison')).toHaveAttribute('data-count', '3');
  });

  it('marks the entry with the lowest Brier as the best', () => {
    render(<ModelComparison open onClose={() => {}} entries={[E1, E2, E3]} />);
    // "★ best" 徽章出现在 best 列上
    // (E3 的 Brier 0.16 最低)
    const best = screen.getByTestId('model-comparison-best');
    expect(best).toBeInTheDocument();
    // best 列就是 E3
    const cols = screen.getAllByTestId('model-comparison-col');
    const bestCol = cols.find((c) => c.getAttribute('data-best') === 'true');
    expect(bestCol).not.toBeNull();
    expect(bestCol).toHaveAttribute('data-job-id', 'c');
  });

  it('shows the lowest Brier summary at the bottom', () => {
    render(<ModelComparison open onClose={() => {}} entries={[E1, E2]} />);
    const summary = screen.getByTestId('model-comparison-summary');
    expect(summary).toBeInTheDocument();
    // summary 包含最佳 model_version
    expect(summary.textContent).toContain('logistic-b-t0');
    expect(summary.textContent).toContain('0.1800');
  });

  it('handles entries with null brier (pre-v0.18)', () => {
    const E_NO_BRIER: PromoteHistoryEntry = {
      ...E1,
      job_id: 'no-brier',
      model_version: 'logistic-no-brier',
      best_brier: null,
    };
    render(<ModelComparison open onClose={() => {}} entries={[E_NO_BRIER, E2]} />);
    // E2(0.18)为 best
    const cols = screen.getAllByTestId('model-comparison-col');
    expect(cols[0]).toHaveAttribute('data-best', 'false'); // null brier
    expect(cols[1]).toHaveAttribute('data-best', 'true'); // 0.18
  });

  // v0.42e-3 —— 来自 archive 的权重
  it('shows weights (w0, w1, w2) when weightsByJobId has a match', () => {
    const weights = new Map<string, { w0: number; w1: number; w2: number }>();
    weights.set('a', { w0: -0.5, w1: 2.0, w2: 0.4 });
    render(
      <ModelComparison
        open
        onClose={() => {}}
        entries={[E1, E2]}
        weightsByJobId={weights}
      />,
    );
    const weightSections = screen.getAllByTestId('model-comparison-weights');
    expect(weightSections).toHaveLength(2);
    // 第一个 entry(job_id='a')有权重
    expect(weightSections[0].textContent).toContain('w0=-0.500');
    expect(weightSections[0].textContent).toContain('w1=2.000');
    expect(weightSections[0].textContent).toContain('w2=0.400');
    // 第二个 entry(job_id='b')没有权重 —— 回退文本
    expect(weightSections[1].textContent).toContain('compare.weights_missing');
  });

  it('shows loading hint when weightsLoading=true and no match', () => {
    render(
      <ModelComparison
        open
        onClose={() => {}}
        entries={[E1, E2]}
        weightsByJobId={new Map()}
        weightsLoading
      />,
    );
    const weightSections = screen.getAllByTestId('model-comparison-weights');
    expect(weightSections[0].textContent).toContain('compare.weights_loading');
  });
});
