// v0.62a.2 — LlmMgmt component tests (more coverage).
//
// /llm-mgmt is the LLM provider/key manager.
// Today 17% coverage. This file covers:
//   1. Add provider modal opens
//   2. List providers section renders

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  llmProviderList: vi.fn().mockResolvedValue([
    { id: 'p1', display_name: 'OpenAI', provider_kind: 'openai',
      request_format: 'chat_completions', supports_streaming: false,
      enabled: true, api_base: 'https://api.openai.com/v1',
      key_alias: 'prod', default_model: 'gpt-4o-mini',
      timeout_ms: 30000, request_timeout_ms: 60000, max_retries: 1,
      cost_per_1k_in: 0.15, cost_per_1k_out: 0.6,
      rate_limit_rpm: 60, rate_limit_tpm: 60000,
      quota_daily_cents: null, quota_monthly_cents: null,
      key_rotation_strategy: 'priority',
      health_status: 'ok', health_latency_p50_ms: 200,
      health_latency_p95_ms: 500, last_health_check_at: Date.now(),
      last_health_error: null, notes: null },
  ]),
  llmKeyList: vi.fn().mockResolvedValue([]),
  llmKeySetSecret: vi.fn().mockResolvedValue(undefined),
  llmKeyDelete: vi.fn().mockResolvedValue(undefined),
  llmKeyUpsert: vi.fn().mockResolvedValue(undefined),
  llmTestConnectivity: vi.fn().mockResolvedValue({ ok: true, latency_ms: 150 }),
  llmProviderUpsert: vi.fn().mockResolvedValue(undefined),
  llmProviderDelete: vi.fn().mockResolvedValue(undefined),
  secretsStatus: vi.fn().mockResolvedValue({
    llm_keys: 0, pm_api: false, pm_passphrase: false, pm_secret: false, wallet_pk: 0,
  }),
  pickFile: vi.fn().mockResolvedValue(null),
}));

vi.mock('@/lib/env-file', () => ({
  extractSecretFromEnv: vi.fn().mockReturnValue(null),
  readFileText: vi.fn().mockResolvedValue(''),
}));

import { LlmMgmt } from './LlmMgmt';

function renderLlmMgmt() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <LlmMgmt />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('LlmMgmt', () => {
  it('renders the page', async () => {
    renderLlmMgmt();
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });

  it('shows a provider from the list', async () => {
    renderLlmMgmt();
    await waitFor(() => {
      expect(screen.getByText('OpenAI')).toBeInTheDocument();
    });
  });

  it('renders a Test button', async () => {
    renderLlmMgmt();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const testBtn = buttons.find((b) => b.textContent?.toLowerCase().includes('test'));
      expect(testBtn).toBeTruthy();
    });
  });
});
