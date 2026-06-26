// v0.106 — StepProgress.tsx 覆盖率提升第 15 轮。
//
// 目标: 覆盖第 32 和 37 行未覆盖的 2 个分支
// (来自 v0.105-final 覆盖率报告)。
//
// 第 32 行: `isPast && 'bg-accent'` — 当 i < currentIndex 时触发。
// 第 37 行: `data-state={isCurrent ? 'current' : isPast ? 'past' : 'future'}` —
//   当 i < currentIndex 且 i !== currentIndex 时触发 `isPast` 分支
//   (currentIndex = 0 时无 past)。
//
// 策略: 用中间步骤作为 current 渲染 (currentIndex = 2),
// 此时: 2 个 past 点, 1 个 current 点, 3 个 future 点。
// 全部 3 种 class 状态均覆盖。

// @vitest-environment happy-dom

import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { StepProgress } from './StepProgress';
import type { WelcomeStep } from '@/stores/welcome-store';

const STEPS: WelcomeStep[] = [
  'welcome', 'storage', 'theme', 'llm', 'polymarket', 'finish',
];

describe('StepProgress round 15', () => {
  it('renders 6 dots with correct data-testid', () => {
    render(<StepProgress current="llm" steps={STEPS} />);
    for (const s of STEPS) {
      expect(screen.getByTestId(`welcome-step-dot-${s}`)).toBeInTheDocument();
    }
  });

  it('marks past dots with state="past" (covers line 32/37 isPast branch)', () => {
    render(<StepProgress current="llm" steps={STEPS} />);
    // 'llm'(currentIndex=3)之前的步骤:'welcome'、'storage'、'theme' → past
    expect(screen.getByTestId('welcome-step-dot-welcome').getAttribute('data-state')).toBe('past');
    expect(screen.getByTestId('welcome-step-dot-storage').getAttribute('data-state')).toBe('past');
    expect(screen.getByTestId('welcome-step-dot-theme').getAttribute('data-state')).toBe('past');
  });

  it('marks current dot with state="current"', () => {
    render(<StepProgress current="llm" steps={STEPS} />);
    expect(screen.getByTestId('welcome-step-dot-llm').getAttribute('data-state')).toBe('current');
  });

  it('marks future dots with state="future"', () => {
    render(<StepProgress current="llm" steps={STEPS} />);
    // 'llm' 之后的步骤:'polymarket'、'finish' → future
    expect(screen.getByTestId('welcome-step-dot-polymarket').getAttribute('data-state')).toBe('future');
    expect(screen.getByTestId('welcome-step-dot-finish').getAttribute('data-state')).toBe('future');
  });

  it('handles first step (currentIndex=0, no past)', () => {
    render(<StepProgress current="welcome" steps={STEPS} />);
    expect(screen.getByTestId('welcome-step-dot-welcome').getAttribute('data-state')).toBe('current');
    expect(screen.getByTestId('welcome-step-dot-storage').getAttribute('data-state')).toBe('future');
  });

  it('handles last step (currentIndex=5, all past except last)', () => {
    render(<StepProgress current="finish" steps={STEPS} />);
    expect(screen.getByTestId('welcome-step-dot-finish').getAttribute('data-state')).toBe('current');
    expect(screen.getByTestId('welcome-step-dot-welcome').getAttribute('data-state')).toBe('past');
  });
});
