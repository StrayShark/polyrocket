import { describe, it, expect } from 'vitest';
import { formatRetentionAge } from './format';

describe('formatRetentionAge (v0.13c)', () => {
  it('returns em-dash for null / undefined / negative', () => {
    expect(formatRetentionAge(null)).toBe('—');
    expect(formatRetentionAge(undefined)).toBe('—');
    expect(formatRetentionAge(-1)).toBe('—');
  });

  it('formats minutes when under an hour', () => {
    expect(formatRetentionAge(60_000)).toBe('1 min');
    expect(formatRetentionAge(30 * 60_000)).toBe('30 min');
  });

  it('formats hours when under a day', () => {
    expect(formatRetentionAge(60 * 60_000)).toBe('1 hours');
    expect(formatRetentionAge(90 * 60_000)).toBe('1.5 hours');
  });

  it('formats days when under 60 days', () => {
    expect(formatRetentionAge(7 * 86_400_000)).toBe('7 days');
    expect(formatRetentionAge(1.5 * 86_400_000)).toBe('1.5 days');
  });

  it('formats months when under 2 years', () => {
    expect(formatRetentionAge(90 * 86_400_000)).toBe('3.0 months');
  });

  it('formats years when 2+ years', () => {
    expect(formatRetentionAge(3 * 365 * 86_400_000)).toBe('3.0 years');
  });
});
