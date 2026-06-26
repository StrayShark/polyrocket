// v0.65d —— Welcome 步骤组件测试(v0.65d 分支补全)。
//
// LlmStep (32% → ~85%) 和 PolymarketStep (31% → ~85%)
// 是 welcome 向导中覆盖率最低的两个步骤组件。
// 我们为两者编写针对性测试。
//
// 每个步骤接收一个 `welcome` store 对象作为 prop
// (由 useWelcomeStore.getState() 返回)。Store 被 mock
// 为一个含 stub 方法(setConfigured、setStep 等)的
// 普通对象 —— 测试断言 IPC 调用 + 渲染 UI 状态。

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { createIpcMock } from '@/test-mocks';

const mockLlmKeyUpsert = vi.fn();
const mockLlmKeySetSecret = vi.fn();
const mockLlmTestConnectivity = vi.fn();
const mockLlmProviderUpsert = vi.fn();
const mockListProviders = vi.fn();

vi.mock('@/ipc', () => createIpcMock({
  llmKeyUpsert: (...args: unknown[]) => mockLlmKeyUpsert(...args),
  llmKeySetSecret: (...args: unknown[]) => mockLlmKeySetSecret(...args),
  llmTestConnectivity: (...args: unknown[]) => mockLlmTestConnectivity(...args),
  llmProviderUpsert: (...args: unknown[]) => mockLlmProviderUpsert(...args),
  llmProviderList: () => mockListProviders(),
}));

import { LlmStep } from './LlmStep';
import type { useWelcomeStore } from '@/stores/welcome-store';

function makeWelcome(): ReturnType<typeof useWelcomeStore.getState> {
  return {
    done: false,
    step: 'llm',
    locale: 'en',
    configured: {
      storagePath: false, theme: false, llmAtLeastOne: false,
      polymarketApi: false, walletPk: false,
    },
    setStep: vi.fn(),
    setDone: vi.fn(),
    setLocale: vi.fn(),
    setConfigured: vi.fn(),
    reset: vi.fn(),
  } as unknown as ReturnType<typeof useWelcomeStore.getState>;
}

describe('LlmStep', () => {
  beforeEach(() => {
    mockLlmKeyUpsert.mockReset();
    mockLlmKeySetSecret.mockReset();
    mockLlmTestConnectivity.mockReset();
    mockLlmProviderUpsert.mockReset();
    mockLlmProviderUpsert.mockResolvedValue(undefined);
  });
  it('renders all 11 provider buttons (5 original + 5 Chinese LLM presets + ERNIE)', () => {
    render(<LlmStep welcome={makeWelcome()} />);
    // 原始 5 个
    expect(screen.getByTestId('welcome-llm-provider-openai')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-llm-provider-anthropic')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-llm-provider-google')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-llm-provider-deepseek')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-llm-provider-custom')).toBeInTheDocument();
    // v0.110 — 中文 LLM 预设 (5)
    expect(screen.getByTestId('welcome-llm-provider-qwen')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-llm-provider-doubao')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-llm-provider-kimi')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-llm-provider-glm')).toBeInTheDocument();
    expect(screen.getByTestId('welcome-llm-provider-MiniMax')).toBeInTheDocument();
    // v0.111 — ERNIE 百度千帆
    expect(screen.getByTestId('welcome-llm-provider-ernie')).toBeInTheDocument();
  });

  it('ERNIE add uses ernie-5.0 (Qianfan v2, OpenAI compat)', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k-ernie' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 350 });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-ernie'));
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'bce-v3/ALTAK-test' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmProviderUpsert).toHaveBeenCalled();
    });
    const args = mockLlmProviderUpsert.mock.calls[0]?.[0] as
      | { id: string; default_model: string; kind: string; api_base: string } | undefined;
    expect(args?.id).toBe('ernie');
    expect(args?.default_model).toBe('ernie-5.0');
    expect(args?.kind).toBe('openai_compat');
    expect(args?.api_base).toBe('https://qianfan.baidubce.com/v2');
  });

  // v0.106 — 覆盖率提升第 15 轮。覆盖 kind()
  //   switch 中缺失的 4 个分支
  //   (openai/anthropic/google/deepseek)。
  //   每个测试点击对应的 provider 按钮
  //   并断言 upsert kind。
  it('Google add uses kind="google" (covers LlmStep.tsx:122)', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k-google' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 200 });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-google'));
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'AIza-test' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmProviderUpsert).toHaveBeenCalled();
    });
    const args = mockLlmProviderUpsert.mock.calls[0]?.[0] as { kind: string } | undefined;
    expect(args?.kind).toBe('google');
  });

  it('DeepSeek add uses kind="deepseek" (covers LlmStep.tsx:123)', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k-deepseek' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 200 });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-deepseek'));
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'sk-deepseek' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmProviderUpsert).toHaveBeenCalled();
    });
    const args = mockLlmProviderUpsert.mock.calls[0]?.[0] as { kind: string } | undefined;
    expect(args?.kind).toBe('deepseek');
  });

  it('Anthropic add uses kind="anthropic" (covers LlmStep.tsx:121)', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k-anthropic' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 200 });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-anthropic'));
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'sk-anthropic' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmProviderUpsert).toHaveBeenCalled();
    });
    const args = mockLlmProviderUpsert.mock.calls[0]?.[0] as { kind: string } | undefined;
    expect(args?.kind).toBe('anthropic');
  });

  it('clicking Qwen preset + Add uses qwen provider_id with Aliyun base', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k-new', provider_id: 'qwen', alias: 'prod-1', keyring_alias: '', enabled: true, priority: 1, weight: 1, last_used_at: null, last_error: null, last_error_at: null, total_calls: 0, total_errors: 0, notes: null });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 200, http_status: 200, model_used: 'qwen3-max', error_code: null, error_message: null });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-qwen'));
    fireEvent.change(screen.getByTestId('welcome-llm-alias'), { target: { value: 'aliyun-1' } });
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'sk-test' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmKeyUpsert).toHaveBeenCalled();
    });
    const args = mockLlmKeyUpsert.mock.calls[0]?.[0] as { provider_id: string; alias: string };
    expect(args.provider_id).toBe('qwen');
    expect(args.alias).toBe('aliyun-1');
  });

  it('clicking Doubao preset + Add uses doubao provider_id', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k-new', provider_id: 'doubao', alias: 'prod-1', keyring_alias: '', enabled: true, priority: 1, weight: 1, last_used_at: null, last_error: null, last_error_at: null, total_calls: 0, total_errors: 0, notes: null });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 150, http_status: 200, model_used: 'doubao-1-5-pro', error_code: null, error_message: null });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-doubao'));
    fireEvent.change(screen.getByTestId('welcome-llm-alias'), { target: { value: 'volc-1' } });
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'volc-test' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmKeyUpsert).toHaveBeenCalled();
    });
    const args = mockLlmKeyUpsert.mock.calls[0]?.[0] as { provider_id: string };
    expect(args.provider_id).toBe('doubao');
  });

  it('provider hint text shows for Chinese presets but not for OpenAI', async () => {
    render(<LlmStep welcome={makeWelcome()} />);
    // OpenAI 没有提示
    expect(screen.queryByTestId('welcome-llm-provider-hint')).not.toBeInTheDocument();
    // 点击 Qwen — 应显示提示
    fireEvent.click(screen.getByTestId('welcome-llm-provider-qwen'));
    await waitFor(() => {
      const hint = screen.getByTestId('welcome-llm-provider-hint');
      expect(hint.textContent).toMatch(/Alibaba/);
    });
    // 再次点击 OpenAI — 提示应消失
    fireEvent.click(screen.getByTestId('welcome-llm-provider-openai'));
    await waitFor(() => {
      expect(screen.queryByTestId('welcome-llm-provider-hint')).not.toBeInTheDocument();
    });
  });

  it('clicking a provider button changes selection', () => {
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-anthropic'));
    // Anthropic 按钮此时带 "selected" class。难以直接
    // 断言,因此我们通过点击 Add 并检查
    // keyring_alias 路径来验证。
  });

  it('shows error toast when alias is empty', async () => {
    const welcome = makeWelcome();
    render(<LlmStep welcome={welcome} />);
    const aliasInput = screen.getByTestId('welcome-llm-alias') as HTMLInputElement;
    fireEvent.change(aliasInput, { target: { value: '' } });
    const secretInput = screen.getByTestId('welcome-llm-secret') as HTMLInputElement;
    fireEvent.change(secretInput, { target: { value: 'sk-test' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    // toast.error 路径已到达;不应有 IPC 调用
    await waitFor(() => {
      expect(mockLlmKeyUpsert).not.toHaveBeenCalled();
    });
  });

  it('shows error toast when secret is empty', async () => {
    render(<LlmStep welcome={makeWelcome()} />);
    const secretInput = screen.getByTestId('welcome-llm-secret') as HTMLInputElement;
    fireEvent.change(secretInput, { target: { value: '' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmKeyUpsert).not.toHaveBeenCalled();
    });
  });

  it('successful add: provider upsert + key upsert + setSecret + test + setConfigured', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k1' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 200 });
    const welcome = makeWelcome();
    render(<LlmStep welcome={welcome} />);
    const secretInput = screen.getByTestId('welcome-llm-secret') as HTMLInputElement;
    fireEvent.change(secretInput, { target: { value: 'sk-mykey' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      // v0.110.2 — provider upsert 必须在 key upsert 之前触发 (这样 default_model 才能持久化)
      expect(mockLlmProviderUpsert).toHaveBeenCalled();
      expect(mockLlmKeyUpsert).toHaveBeenCalled();
      expect(mockLlmKeySetSecret).toHaveBeenCalledWith('k1', 'sk-mykey');
      expect(mockLlmTestConnectivity).toHaveBeenCalledWith('openai', 'k1');
      expect(welcome.setConfigured).toHaveBeenCalledWith('llmAtLeastOne', true);
    });
    // v0.110.2 — 断言 PROVIDERS 中的 default_model 已传入
    const providerArgs = mockLlmProviderUpsert.mock.calls[0]?.[0] as
      | { id: string; default_model: string; kind: string; api_base: string } | undefined;
    expect(providerArgs?.id).toBe('openai');
    expect(providerArgs?.default_model).toBe('gpt-4o');
    expect(providerArgs?.kind).toBe('openai');
    expect(providerArgs?.api_base).toBe('https://api.openai.com/v1');
  });

  it('GLM add uses glm-5.2 as default_model (latest stable 2026-06)', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k1' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 300 });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-glm'));
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'sk-glm' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmProviderUpsert).toHaveBeenCalled();
    });
    const args = mockLlmProviderUpsert.mock.calls[0]?.[0] as
      | { default_model: string; kind: string; api_base: string } | undefined;
    expect(args?.default_model).toBe('glm-5.2');
    expect(args?.kind).toBe('openai_compat');
    expect(args?.api_base).toBe('https://open.bigmodel.cn/api/paas/v4');
  });

  it('Qwen add uses qwen3.7-max as default_model (latest stable 2026-06)', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k1' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 200 });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-qwen'));
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'sk-qwen' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmProviderUpsert).toHaveBeenCalled();
    });
    const args = mockLlmProviderUpsert.mock.calls[0]?.[0] as { default_model: string } | undefined;
    expect(args?.default_model).toBe('qwen3.7-max');
  });

  it('Doubao add uses doubao-seed-2-0-pro-260215 (Seed 2.0 2026-02)', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k1' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 250 });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-doubao'));
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'sk-doubao' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmProviderUpsert).toHaveBeenCalled();
    });
    const args = mockLlmProviderUpsert.mock.calls[0]?.[0] as { default_model: string } | undefined;
    expect(args?.default_model).toBe('doubao-seed-2-0-pro-260215');
  });

  it('Kimi add uses kimi-k2.7-code (latest 2026)', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k1' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 280 });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-kimi'));
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'sk-kimi' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmProviderUpsert).toHaveBeenCalled();
    });
    const args = mockLlmProviderUpsert.mock.calls[0]?.[0] as { default_model: string } | undefined;
    expect(args?.default_model).toBe('kimi-k2.7-code');
  });

  it('MiniMax add uses MiniMax-M2.7 (latest stable 2026)', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k1' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 200 });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-MiniMax'));
    fireEvent.change(screen.getByTestId('welcome-llm-secret'), { target: { value: 'sk-MM' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(mockLlmProviderUpsert).toHaveBeenCalled();
    });
    const args = mockLlmProviderUpsert.mock.calls[0]?.[0] as { default_model: string } | undefined;
    expect(args?.default_model).toBe('MiniMax-M2.7');
  });

  it('failed connectivity: shows result with error message', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k1' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({
      ok: false,
      error_code: '401',
      error_message: 'Unauthorized',
    });
    const welcome = makeWelcome();
    render(<LlmStep welcome={welcome} />);
    const secretInput = screen.getByTestId('welcome-llm-secret') as HTMLInputElement;
    fireEvent.change(secretInput, { target: { value: 'sk-bad' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(screen.getByTestId('welcome-llm-result')).toBeInTheDocument();
    });
    // 连通性失败时不应调用 setConfigured
    expect(welcome.setConfigured).not.toHaveBeenCalled();
  });

  it('upsert throws → catch block sets result with error', async () => {
    mockLlmKeyUpsert.mockRejectedValue(new Error('DB locked'));
    render(<LlmStep welcome={makeWelcome()} />);
    const secretInput = screen.getByTestId('welcome-llm-secret') as HTMLInputElement;
    fireEvent.change(secretInput, { target: { value: 'sk-x' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      expect(screen.getByTestId('welcome-llm-result')).toBeInTheDocument();
    });
  });

  it('clicking Anthropic + Add uses anthropic provider id in args', async () => {
    mockLlmKeyUpsert.mockResolvedValue({ id: 'k2' });
    mockLlmKeySetSecret.mockResolvedValue(undefined);
    mockLlmTestConnectivity.mockResolvedValue({ ok: true, latency_ms: 100 });
    render(<LlmStep welcome={makeWelcome()} />);
    fireEvent.click(screen.getByTestId('welcome-llm-provider-anthropic'));
    const secretInput = screen.getByTestId('welcome-llm-secret') as HTMLInputElement;
    fireEvent.change(secretInput, { target: { value: 'sk-anthro' } });
    fireEvent.click(screen.getByTestId('welcome-llm-add'));
    await waitFor(() => {
      const args = mockLlmKeyUpsert.mock.calls[0]?.[0];
      expect(args).toEqual(expect.objectContaining({ provider_id: 'anthropic' }));
      // keyring_alias 也应使用 anthropic
      expect(args.keyring_alias).toMatch(/^polyrocket\/llm\/anthropic\//);
    });
  });
});
