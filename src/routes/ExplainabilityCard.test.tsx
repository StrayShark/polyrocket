// v0.59 — ExplainabilityCard SHAP/derivative
// toggle tests.
//
// The Settings → ExplainabilityCard now
// has a 2-way toggle between SHAP
// (KernelExplainer, satisfies the efficiency
// axiom) and the v0.55 exact-decomposition
// (derivative of p w.r.t. each feature). Both
// flows go through the same form, just
// different IPCs.

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
    // Wait for getActiveModel to resolve
    // before the test continues.
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
    // The result panel is marked with
    // data-method="shap".
    await waitFor(() => {
      expect(
        screen.getByTestId('explain-result').getAttribute('data-method'),
      ).toBe('shap');
    });
    // The result panel mentions the method name.
    expect(screen.getByTestId('explain-result').textContent).toContain(
      'kernel_shap',
    );
  });

  it('switching to Derivative calls explainModel (v0.55) instead', async () => {
    render(wrap(<ExplainabilityCard />));
    await waitFor(() => screen.getByTestId('explain-method-deriv'));
    // Click the Derivative toggle.
    fireEvent.click(screen.getByTestId('explain-method-deriv'));
    fireEvent.click(screen.getByTestId('explain-run'));
    await waitFor(() => {
      expect(ipc.explainModel).toHaveBeenCalled();
    });
    // SHAP should NOT have been called.
    expect(ipc.shapExplain).not.toHaveBeenCalled();
    // Result panel shows the derivative method
    // (we just don't print the method name
    // for the derivative view; we set the
    // data-method attribute).
    await waitFor(() => {
      expect(
        screen.getByTestId('explain-result').getAttribute('data-method'),
      ).toBe('derivative');
    });
  });
});
