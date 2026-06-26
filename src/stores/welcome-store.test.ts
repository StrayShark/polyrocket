// v0.53c — welcome store 的 vitest 覆盖率。
//
// 1. 默认值正确
// 2. setStep / setDone / setConfigured 状态转换
// 3. pendingConfigs 返回正确的 keys
// 4. 旧版 polyrocket.onboarding localStorage key
//    在首次读取时被迁移

import { describe, it, expect, beforeEach } from 'vitest';
import {
  useWelcomeStore,
  WELCOME_STEPS,
  pendingConfigs,
} from './welcome-store';

beforeEach(() => {
  // 在每个测试之间重置 store。
  useWelcomeStore.getState().reset();
  // 清空 localStorage，确保旧版迁移
  // 测试从一个干净的状态开始。
  if (typeof window !== 'undefined') {
    window.localStorage.clear();
  }
});

describe('welcome store defaults', () => {
  it('starts on the welcome step', () => {
    const s = useWelcomeStore.getState();
    expect(s.step).toBe('welcome');
    expect(s.done).toBe(false);
    expect(s.configured).toEqual({
      storagePath: false,
      theme: false,
      llmAtLeastOne: false,
      polymarketApi: false,
      walletPk: false,
    });
  });

  it('exposes 6 step names in order', () => {
    expect(WELCOME_STEPS).toEqual([
      'welcome',
      'storage',
      'theme',
      'llm',
      'polymarket',
      'finish',
    ]);
  });
});

describe('welcome store transitions', () => {
  it('setStep advances the step', () => {
    useWelcomeStore.getState().setStep('llm');
    expect(useWelcomeStore.getState().step).toBe('llm');
  });

  it('setDone=true jumps to finish', () => {
    useWelcomeStore.getState().setStep('llm');
    useWelcomeStore.getState().setDone(true);
    const s = useWelcomeStore.getState();
    expect(s.done).toBe(true);
    expect(s.step).toBe('finish');
  });

  it('setDone=false resets to welcome', () => {
    useWelcomeStore.getState().setStep('polymarket');
    useDone();
    function useDone() {
      useWelcomeStore.getState().setDone(false);
    }
    const s = useWelcomeStore.getState();
    expect(s.step).toBe('welcome');
  });

  it('setConfigured updates the matching flag', () => {
    const s = useWelcomeStore.getState();
    s.setConfigured('llmAtLeastOne', true);
    s.setConfigured('polymarketApi', true);
    expect(useWelcomeStore.getState().configured.llmAtLeastOne).toBe(true);
    expect(useWelcomeStore.getState().configured.polymarketApi).toBe(true);
    // 其他 flag 保持 false。
    expect(useWelcomeStore.getState().configured.walletPk).toBe(false);
  });
});

describe('pendingConfigs', () => {
  it('returns the list of unconfigured items', () => {
    const s = useWelcomeStore.getState();
    // 尚未配置任何项。
    expect(pendingConfigs(s)).toEqual(['llm', 'polymarket', 'wallet']);
    s.setConfigured('llmAtLeastOne', true);
    expect(pendingConfigs(useWelcomeStore.getState())).toEqual([
      'polymarket',
      'wallet',
    ]);
    s.setConfigured('polymarketApi', true);
    s.setConfigured('walletPk', true);
    expect(pendingConfigs(useWelcomeStore.getState())).toEqual([]);
  });

  it('storage + theme are NOT in the pending list (they\'re setup, not secrets)', () => {
    const s = useWelcomeStore.getState();
    // 即使未配置，storage/theme 也不会出现
    // ——它们是 nice-to-have，不是阻塞项。用户
    // 无需选择自定义路径或非默认主题即可完成向导。
    expect(pendingConfigs(s)).not.toContain('storage');
    expect(pendingConfigs(s)).not.toContain('theme');
  });
});

describe('legacy polyrocket.onboarding migration', () => {
  it('copies done + step from the old key on first read', () => {
    if (typeof window === 'undefined') return;
    window.localStorage.setItem(
      'polyrocket.onboarding',
      JSON.stringify({
        state: { done: true, step: 3 },
        version: 0,
      }),
    );
    // 通过读取触发迁移。
    useWelcomeStore.persist.rehydrate();
    // 迁移之后，旧 key 已删除，新 key
    // 存在，done=true 且 step 设置为
    // 一个合理的 WelcomeStep。
    expect(window.localStorage.getItem('polyrocket.onboarding')).toBeNull();
    const newRaw = window.localStorage.getItem('polyrocket.welcome');
    expect(newRaw).not.toBeNull();
    const parsed = JSON.parse(newRaw as string);
    expect(parsed.state.done).toBe(true);
    // 旧版 step 3 (llm) 映射到 'llm'。
    expect(parsed.state.step).toBe('llm');
  });
});
