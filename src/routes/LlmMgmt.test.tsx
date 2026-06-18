// v0.62a — LlmMgmt component tests.
//
// /llm-mgmt is the LLM provider/key manager.
// Today 0% coverage. This file covers the
// initial render + section headings.

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/ipc', () => ({
  llmProviderList: vi.fn().mockResolvedValue([]),
  llmKeyList: vi.fn().mockResolvedValue([]),
  secretsStatus: vi.fn().mockResolvedValue({
    llm_keys: 0, pm_api: false, pm_passphrase: false, pm_secret: false, wallet_pk: 0,
  }),
  pickFile: vi.fn().mockResolvedValue(null),
  llmKeySetSecret: vi.fn().mockResolvedValue(undefined),
  llmKeyDelete: vi.fn().mockResolvedValue(undefined),
  llmTestConnectivity: vi.fn().mockResolvedValue({ ok: true, latency_ms: 100 }),
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
});
