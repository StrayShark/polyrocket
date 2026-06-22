// v0.106 — StepProgress.tsx coverage ramp round 15.
//
// Target: cover the 2 uncovered branches at lines 32 and 37
// (from coverage report at v0.105-final).
//
// Lines 32: `isPast && 'bg-accent'` — fires when i < currentIndex.
// Lines 37: `data-state={isCurrent ? 'current' : isPast ? 'past' : 'future'}` —
//   the `isPast` branch fires when i < currentIndex but i !== currentIndex
//   (currentIndex = 0 means no past).
//
// Strategy: render with current set to a middle step (currentIndex = 2),
// so we have: 2 past dots, 1 current dot, 3 future dots. All 3 class states
// covered.

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
    // Steps before 'llm' (currentIndex=3): 'welcome', 'storage', 'theme' → past
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
    // Steps after 'llm': 'polymarket', 'finish' → future
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
