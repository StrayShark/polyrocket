// @vitest-environment happy-dom
import { describe, expect, it, vi, beforeAll, afterAll } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ModelVersionPill } from './ModelVersionPill';

beforeAll(() => { vi.spyOn(console, 'error').mockImplementation(() => {}); });
afterAll(() => { vi.restoreAllMocks(); });

describe('ModelVersionPill', () => {
  it('renders a known model_version', () => {
    render(<ModelVersionPill modelVersion="logistic-train-441c352b" />);
    const pill = screen.getByTestId('model-version-pill');
    expect(pill).toBeInTheDocument();
    expect(pill).toHaveAttribute('data-version', 'logistic-train-441c352b');
    expect(pill.textContent).toContain('logistic-train-441c352b');
  });

  it('renders "unknown" when modelVersion is null', () => {
    render(<ModelVersionPill modelVersion={null} />);
    const pill = screen.getByTestId('model-version-pill');
    expect(pill).toHaveAttribute('data-version', 'unknown');
    expect(pill.textContent).toContain('unknown');
  });

  it('renders "unknown" when modelVersion is undefined', () => {
    render(<ModelVersionPill modelVersion={undefined} />);
    expect(screen.getByTestId('model-version-pill')).toHaveAttribute('data-version', 'unknown');
  });

  it('renders "unknown" when modelVersion is empty string', () => {
    render(<ModelVersionPill modelVersion="" />);
    expect(screen.getByTestId('model-version-pill')).toHaveAttribute('data-version', 'unknown');
  });

  it('prefixes "scoring with" in verbose mode', () => {
    render(<ModelVersionPill modelVersion="logistic-0.1.0" variant="verbose" />);
    const pill = screen.getByTestId('model-version-pill');
    expect(pill.textContent).toContain('scoring with');
    expect(pill.textContent).toContain('logistic-0.1.0');
  });

  it('does not prefix in compact mode (default)', () => {
    render(<ModelVersionPill modelVersion="logistic-0.1.0" />);
    const pill = screen.getByTestId('model-version-pill');
    expect(pill.textContent).not.toContain('scoring with');
    expect(pill.textContent).toContain('logistic-0.1.0');
  });
});
