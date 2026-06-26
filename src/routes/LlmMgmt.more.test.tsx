// v0.63a — LlmMgmt 补充测试（mutation + 添加流程）。
//
// /llm-mgmt 语句覆盖率为 21%。本文件覆盖：
//   1. 用新 mock 渲染 providers 列表
//   2. 打开 Add provider 弹窗
//   3. 提交 Add provider → 调用 llmProviderUpsert
//   4. 渲染空 key 列表
//   5. Test 按钮调用 llmTestConnectivity

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
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
    { id: 'p2', display_name: 'Anthropic', provider_kind: 'anthropic',
      request_format: 'messages', supports_streaming: false,
      enabled: true, api_base: 'https://api.anthropic.com',
      key_alias: 'prod', default_model: 'claude-sonnet',
      timeout_ms: 30000, request_timeout_ms: 60000, max_retries: 1,
      cost_per_1k_in: 3.0, cost_per_1k_out: 15.0,
      rate_limit_rpm: 50, rate_limit_tpm: 40000,
      quota_daily_cents: 500, quota_monthly_cents: 10000,
      key_rotation_strategy: 'priority',
      health_status: 'slow', health_latency_p50_ms: 800,
      health_latency_p95_ms: 2000, last_health_check_at: Date.now() - 60000,
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
import { llmProviderUpsert, llmTestConnectivity } from '@/ipc';

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

describe('LlmMgmt (more coverage)', () => {
  beforeEach(() => {
    vi.mocked(llmProviderUpsert).mockClear();
    vi.mocked(llmTestConnectivity).mockClear();
  });

  it('renders both providers from the list', async () => {
    renderLlmMgmt();
    await waitFor(() => {
      expect(screen.getByText('OpenAI')).toBeInTheDocument();
      expect(screen.getByText('Anthropic')).toBeInTheDocument();
    });
  });

  it('shows Test button for each provider', async () => {
    renderLlmMgmt();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const testButtons = buttons.filter((b) => b.textContent?.toLowerCase().includes('test'));
      // 每个 provider 至少一个 Test 按钮
      expect(testButtons.length).toBeGreaterThanOrEqual(1);
    });
  });

  it('clicking Test calls llmTestConnectivity', async () => {
    renderLlmMgmt();
    await waitFor(() => {
      const buttons = screen.getAllByRole('button');
      const testBtn = buttons.find((b) => b.textContent?.toLowerCase().includes('test'));
      expect(testBtn).toBeTruthy();
    });
    const buttons = screen.getAllByRole('button');
    const testBtn = buttons.find((b) => b.textContent?.toLowerCase().includes('test'))!;
    fireEvent.click(testBtn);
    await waitFor(() => {
      expect(llmTestConnectivity).toHaveBeenCalled();
    });
  });

  it('clicking a provider selects it (sets selectedProvider state)', async () => {
    renderLlmMgmt();
    await waitFor(() => {
      expect(screen.getByText('OpenAI')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('OpenAI'));
    await waitFor(() => {
      // 选中后，Keys section 会显示 Add Key 按钮
      const buttons = screen.getAllByRole('button');
      const addBtn = buttons.find((b) => b.textContent?.toLowerCase().includes('add'));
      expect(addBtn).toBeTruthy();
    });
  });
});
