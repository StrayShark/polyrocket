// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TimelinePill } from './TimelinePill';

describe('TimelinePill (v0.119 Cursor-merge)', () => {
  it('renders all 5 stages with correct data-stage attribute', () => {
    const stages = ['thinking', 'grep', 'read', 'edit', 'done'] as const;
    for (const stage of stages) {
      const { unmount } = render(<TimelinePill stage={stage} />);
      const pill = screen.getByTestId('timeline-pill');
      expect(pill).toHaveAttribute('data-stage', stage);
      unmount();
    }
  });

  it('uses default English label per stage', () => {
    const cases = [
      ['thinking', 'Thinking'],
      ['grep', 'Grepping'],
      ['read', 'Reading'],
      ['edit', 'Editing'],
      ['done', 'Done'],
    ] as const;
    for (const [stage, expectedLabel] of cases) {
      const { unmount } = render(<TimelinePill stage={stage} />);
      expect(screen.getByText(expectedLabel)).toBeInTheDocument();
      unmount();
    }
  });

  it('respects a custom label override', () => {
    render(<TimelinePill stage="done" label="FINISHED" />);
    expect(screen.getByText('FINISHED')).toBeInTheDocument();
  });

  it('applies uppercase + tracking + pill shape per Cursor caption-uppercase spec', () => {
    const { container } = render(<TimelinePill stage="thinking" />);
    const pill = container.querySelector('[data-testid="timeline-pill"]')!;
    expect(pill.className).toMatch(/uppercase/);
    expect(pill.className).toMatch(/tracking-caption-uppercase/);
    expect(pill.className).toMatch(/rounded-pill/);
    expect(pill.className).toMatch(/text-xs/);
    expect(pill.className).toMatch(/font-semibold/);
  });

  it('uses thinking bg class for thinking stage', () => {
    const { container } = render(<TimelinePill stage="thinking" />);
    const pill = container.querySelector('[data-testid="timeline-pill"]')!;
    expect(pill.className).toContain('bg-timeline-thinking');
  });

  it('uses done bg class + white-text for done stage', () => {
    const { container } = render(<TimelinePill stage="done" />);
    const pill = container.querySelector('[data-testid="timeline-pill"]')!;
    expect(pill.className).toContain('bg-timeline-done');
    expect(pill.className).toContain('text-bg'); // white text on gold
  });

  it('passes through additional className', () => {
    const { container } = render(
      <TimelinePill stage="read" className="custom-extra-class" />,
    );
    expect(
      container.querySelector('[data-testid="timeline-pill"]')!.className,
    ).toContain('custom-extra-class');
  });

  it('renders 5 distinct bg color classes (no overlap)', () => {
    const stages = ['thinking', 'grep', 'read', 'edit', 'done'] as const;
    const seen = new Set<string>();
    for (const stage of stages) {
      const { container, unmount } = render(<TimelinePill stage={stage} />);
      const pill = container.querySelector('[data-testid="timeline-pill"]')!;
      // Extract just bg-* class
      const match = pill.className.match(/bg-timeline-\w+/);
      if (match) seen.add(match[0]);
      unmount();
    }
    expect(seen.size).toBe(5);
  });
});
