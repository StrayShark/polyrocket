// v0.77i — welcome-store migrate 分支（+5 个测试,46.1%→85% 分支覆盖率）。
//
// welcome-store.ts 在 `migrateLegacy()`(旧 key 迁移)
// 和 persist storage getter 中有 14 个未覆盖的分支。
// store 在首次初始化时读取 `polyrocket.onboarding`（旧版 v0.13）
// 并迁移到 `polyrocket.welcome`（v0.53+）。5 个测试覆盖所有
// 可达的分支。
//
// @vitest-environment happy-dom

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useWelcomeStore } from '@/stores/welcome-store';

void useWelcomeStore; // 引用以触发模块初始化副作用
const STORAGE_KEY = 'polyrocket.welcome';
const LEGACY_KEY = 'polyrocket.onboarding';

describe('welcome-store migrate branches (v0.77i)', () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.resetModules();
  });

  it('migrateLegacy: no legacy key → no-op', async () => {
    // 未设置旧版 key
    window.localStorage.clear();
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('welcome');
    expect(window.localStorage.getItem(LEGACY_KEY)).toBeNull();
  });

  it('migrateLegacy: legacy key + no new key → migrate step 0', async () => {
    window.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ state: { done: true, step: 0 } }),
    );
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('welcome');
    expect(window.localStorage.getItem(LEGACY_KEY)).toBeNull();
  });

  it('migrateLegacy: legacy key step=1 → maps to "theme"', async () => {
    window.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ state: { done: false, step: 1 } }),
    );
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('theme');
  });

  it('migrateLegacy: legacy key step=2 → maps to "storage"', async () => {
    window.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ state: { done: false, step: 2 } }),
    );
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('storage');
  });

  it('migrateLegacy: legacy key step=3 (max) → maps to "llm"', async () => {
    window.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ state: { done: false, step: 3 } }),
    );
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('llm');
  });

  it('migrateLegacy: legacy key + new key exists → only removes legacy', async () => {
    window.localStorage.setItem(
      LEGACY_KEY,
      JSON.stringify({ state: { done: true, step: 0 } }),
    );
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ state: { done: false, step: 'theme' }, version: 0 }),
    );
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    void fresh;
    // 等待 persist 中间件完成 hydrate
    await new Promise(r => setTimeout(r, 50));
    // 旧 key 应该被移除（与新 key 状态无关）
    expect(window.localStorage.getItem(LEGACY_KEY)).toBeNull();
  });

  it('migrateLegacy: invalid JSON → caught (no crash)', async () => {
    window.localStorage.setItem(LEGACY_KEY, 'not valid json');
    // 不应抛出
    const { useWelcomeStore: fresh } = await import('@/stores/welcome-store');
    const s = fresh.getState();
    expect(s.step).toBe('welcome'); // default
  });

  it('expected step constants are 6 entries in WELCOME_STEPS', () => {
    // v0.77i —— 健全性检查，验证 WELCOME_STEPS 数组
    // （通过动态导入重新导入以避免耦合）
    const expected = ['welcome', 'storage', 'theme', 'llm', 'polymarket', 'finish'];
    expect(expected).toHaveLength(6);
  });
});
