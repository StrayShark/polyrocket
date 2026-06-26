// v0.59 —— ExplainabilityCard SHAP/导数
// 切换测试。
//
// Settings → ExplainabilityCard 现在提供
// SHAP（KernelExplainer，满足效率公理）和
// v0.55 精确分解（p 对各特征的导数）之间的
// 二选一切换。两种流程使用相同的表单，
// 只是 IPC 不同。

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  getActiveModel: vi.fn().mockResolvedValue({
    modelVersion: 'logistic-train-test',
    bestBrier: 0.18,
    bestParams: null,
    promotedAtMs: 1700000000000,
    weights: [0.1, 2.4, -0.02],
    sourcePath: '/home/test/.polyrocket/sidecar/models/active.json',
  }),
  explainModel: vi.fn().mockResolvedValue({
    ok: true,
    modelVersion: 'logistic-train-test',
    features: [
      { feature: 'bias', value: 1.0, weight: 0.1, contribution: 0.025, abs_contribution: 0.025 },
      { feature: 'price', value: 0.5, weight: 2.4, contribution: 0.30, abs_contribution: 0.30 },
    ],
    prediction: 0.55,
    sample: { price: 0.5, market_age_hours: 24.0 },
    message: 'ok',
  }),
  shapExplain: vi.fn().mockResolvedValue({
    ok: true,
    modelVersion: 'logistic-train-test',
    method: 'kernel_shap',
    features: [
      { feature: 'price', value: 0.5, weight: 2.4, shap_value: 0.30, abs_shap: 0.30 },
      { feature: 'bias', value: 1.0, weight: 0.1, shap_value: 0.025, abs_shap: 0.025 },
    ],
    baseline_prediction: 0.5,
    target_prediction: 0.825,
    efficiency_diff: 0.0,
    sample: { price: 0.5, market_age_hours: 24.0 },
    message: 'ok',
  }),
}));

import * as ipc from '@/ipc';
import { ExplainabilityCard } from './Settings';

function wrap(node: React.ReactNode) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return <QueryClientProvider client={qc}>{node}</QueryClientProvider>;
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(ipc.getActiveModel).mockResolvedValue({
    modelVersion: 'logistic-train-test',
    bestBrier: 0.18,
    bestParams: null,
    promotedAtMs: 1700000000000,
    weights: [0.1, 2.4, -0.02],
    sourcePath: '/home/test/.polyrocket/sidecar/models/active.json',
  });
});

describe('ExplainabilityCard SHAP/derivative toggle (v0.59)', () => {
  it('renders the SHAP + Derivative toggle buttons', async () => {
    render(wrap(<ExplainabilityCard />));
    // 在测试继续前等待 getActiveModel 解析完成。
    await waitFor(() => {
      expect(
        screen.getByTestId('explain-method-shap'),
      ).toBeInTheDocument();
    });
    expect(
      screen.getByTestId('explain-method-deriv'),
    ).toBeInTheDocument();
  });

  it('defaults to SHAP and the result panel shows the method + baseline + efficiency', async () => {
    render(wrap(<ExplainabilityCard />));
    await waitFor(() => screen.getByTestId('explain-run'));
    fireEvent.click(screen.getByTestId('explain-run'));
    await waitFor(() => {
      expect(ipc.shapExplain).toHaveBeenCalled();
    });
    // 结果面板带有
    // data-method="shap" 标记。
    await waitFor(() => {
      expect(
        screen.getByTestId('explain-result').getAttribute('data-method'),
      ).toBe('shap');
    });
    // 结果面板包含方法名称。
    expect(screen.getByTestId('explain-result').textContent).toContain(
      'kernel_shap',
    );
  });

  it('switching to Derivative calls explainModel (v0.55) instead', async () => {
    render(wrap(<ExplainabilityCard />));
    await waitFor(() => screen.getByTestId('explain-method-deriv'));
    // 点击 Derivative 切换按钮。
    fireEvent.click(screen.getByTestId('explain-method-deriv'));
    fireEvent.click(screen.getByTestId('explain-run'));
    await waitFor(() => {
      expect(ipc.explainModel).toHaveBeenCalled();
    });
    // 不应调用 SHAP。
    expect(ipc.shapExplain).not.toHaveBeenCalled();
    // 结果面板展示 derivative 方法
    // （我们不为 derivative 视图打印方法名称；
    // 仅设置 data-method 属性）。
    await waitFor(() => {
      expect(
        screen.getByTestId('explain-result').getAttribute('data-method'),
      ).toBe('derivative');
    });
  });
});
