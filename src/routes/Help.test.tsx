// v0.62a — Help 组件测试。
//
// /help 是文档与快速开始页面。
// 无 IPC，纯渲染。目前覆盖率 0%。

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';

vi.mock('@/lib/i18n', () => ({
  useT: () => ({ t: (k: string) => {
    // Help.tsx 使用 t(...).split('/signals') / .split('/analysis') —— 返回
    // 含有这些标记的字符串，使 split 至少得到 2 个元素。
    if (k === 'help.quick.step5') return 'Go to /signals and /analysis for live data.';
    if (k === 'help.quick.step2') return 'Open /wallets to add a wallet.';
    return k;
  } }),
}));

import { Help } from './Help';

describe('Help', () => {
  it('renders the page', async () => {
    render(
      <MemoryRouter>
        <Help />
      </MemoryRouter>,
    );
    await waitFor(() => {
      // 多处 "help" 文本出现（h2 + 导航）
      expect(screen.getAllByText(/help/i).length).toBeGreaterThan(0);
    });
  });
});
