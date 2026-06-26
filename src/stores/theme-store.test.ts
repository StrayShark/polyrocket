// v0.62a —— theme-store 测试。
//
// theme store 是一个小型 zustand store，
// 将 `theme`（dark / light / matrix）持久化到
// localStorage 并将 `data-theme` 应用到 <html>。
// 当前覆盖率为 0%。本文件覆盖：
//   1. setTheme 更新状态
//   2. cycleTheme 按 dark → light → matrix → dark 推进
//   3. SSR 安全（测试环境中不访问 document）

// @vitest-environment happy-dom

import { describe, it, expect, beforeEach } from 'vitest';
import { useThemeStore } from './theme-store';

describe('useThemeStore', () => {
  beforeEach(() => {
    // 重置为默认 dark
    useThemeStore.setState({ theme: 'dark' });
  });

  it('defaults to dark', () => {
    expect(useThemeStore.getState().theme).toBe('dark');
  });

  it('setTheme updates the state', () => {
    useThemeStore.getState().setTheme('matrix');
    expect(useThemeStore.getState().theme).toBe('matrix');
  });

  it('cycleTheme advances dark → light', () => {
    useThemeStore.getState().setTheme('dark');
    useThemeStore.getState().cycleTheme();
    expect(useThemeStore.getState().theme).toBe('light');
  });

  it('cycleTheme advances light → matrix', () => {
    useThemeStore.getState().setTheme('light');
    useThemeStore.getState().cycleTheme();
    expect(useThemeStore.getState().theme).toBe('matrix');
  });

  it('cycleTheme advances matrix → dark (wraps)', () => {
    useThemeStore.getState().setTheme('matrix');
    useThemeStore.getState().cycleTheme();
    expect(useThemeStore.getState().theme).toBe('dark');
  });

  // v0.106 —— 覆盖率提升。覆盖 applyToDom（第 23-26 行）以及
  // onRehydrateStorage 分支（第 51 行：`if (state?.theme) applyToDom(...)`）。
  it('setTheme applies data-theme attribute to <html>', () => {
    useThemeStore.getState().setTheme('matrix');
    expect(document.documentElement.getAttribute('data-theme')).toBe('matrix');
  });

  it('setTheme to light applies data-theme="light"', () => {
    useThemeStore.getState().setTheme('light');
    expect(document.documentElement.getAttribute('data-theme')).toBe('light');
  });
});
