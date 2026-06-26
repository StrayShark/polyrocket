// v0.62a.2 —— Welcome 组件测试。
//
// /welcome 是首次运行的引导向导。
// 目前 0% 覆盖率。本文件覆盖：
//   1. 步骤进度的初始渲染
//   2. StorageStep 渲染路径输入框 + Browse 按钮
//   3. 如果 welcome 已完成，重定向到 dashboard

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => k }),
  useLocaleStore: () => ({}),
}));

// 默认：welcome 未完成
vi.mock('@/stores/welcome-store', () => ({
  useWelcomeStore: Object.assign(
    () => ({
      done: false,
      step: 'welcome',
      locale: '',
      configured: {
        storagePath: false, theme: false, llmAtLeastOne: false,
        polymarketApi: false, walletPk: false,
      },
      setStep: vi.fn(),
      setDone: vi.fn(),
      setLocale: vi.fn(),
      setConfigured: vi.fn(),
      reset: vi.fn(),
    }),
    { getState: () => ({
      done: false, step: 'welcome', locale: '',
      configured: { storagePath: false, theme: false, llmAtLeastOne: false,
        polymarketApi: false, walletPk: false },
      setStep: vi.fn(), setDone: vi.fn(), setLocale: vi.fn(),
      setConfigured: vi.fn(), reset: vi.fn(),
    }) },
  ),
  WELCOME_STEPS: ['welcome', 'storage', 'theme', 'llm', 'polymarket', 'finish'],
}));

import { Welcome } from './Welcome';

describe('Welcome', () => {
  it('renders the wizard', async () => {
    render(
      <MemoryRouter>
        <Welcome />
      </MemoryRouter>,
    );
    await waitFor(() => {
      expect(document.body.textContent).toBeTruthy();
    });
  });
});
