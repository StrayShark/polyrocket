// v0.94 — format.ts edge case tests (5 tests, +~5 stmts).
//
// Adds tests for fmtRelativeTime, fmtDate, fmtDateTime, fmtAddress
// (the 4 most-used formatters that weren't covered by the existing
// formatRetentionAge-only test file). Each formatter has a `null`
// / `undefined` branch and several magnitude branches.

import { describe, it, expect } from 'vitest';
import {
  formatRetentionAge,
  fmtRelativeTime,
  fmtDate,
  fmtDateTime,
  fmtAddress,
} from './format';

describe('formatRetentionAge (v0.13c — existing)', () => {
  it('returns em-dash for null / undefined / negative', () => {
    expect(formatRetentionAge(null)).toBe('—');
    expect(formatRetentionAge(undefined)).toBe('—');
    expect(formatRetentionAge(-1)).toBe('—');
  });
});

describe('fmtRelativeTime (v0.94)', () => {
  it('returns em-dash for null / undefined', () => {
    expect(fmtRelativeTime(null)).toBe('—');
    expect(fmtRelativeTime(undefined)).toBe('—');
  });

  it('returns "just now" for less than 60 seconds', () => {
    expect(fmtRelativeTime(Date.now() - 5_000)).toBe('just now');
  });

  it('returns minutes for under an hour', () => {
    expect(fmtRelativeTime(Date.now() - 5 * 60_000)).toMatch(/5m ago/);
  });

  it('returns hours for under a day', () => {
    expect(fmtRelativeTime(Date.now() - 3 * 3_600_000)).toMatch(/3h ago/);
  });

  it('returns days for a day or more', () => {
    expect(fmtRelativeTime(Date.now() - 2 * 86_400_000)).toMatch(/2d ago/);
  });
});

describe('fmtDate (v0.94)', () => {
  it('returns em-dash for null / undefined', () => {
    expect(fmtDate(null)).toBe('—');
    expect(fmtDate(undefined)).toBe('—');
  });

  it('formats timestamp as US locale short date', () => {
    // Use Date.now() to avoid timezone drift
    const out = fmtDate(Date.now() - 24 * 3_600_000);
    expect(out).toMatch(/[A-Z][a-z]{2} \d+, \d{4}/);
  });
});

describe('fmtDateTime (v0.94)', () => {
  it('returns em-dash for null / undefined', () => {
    expect(fmtDateTime(null)).toBe('—');
    expect(fmtDateTime(undefined)).toBe('—');
  });

  it('formats timestamp as US locale date+time', () => {
    const out = fmtDateTime(Date.now() - 60_000);
    // Date part
    expect(out).toMatch(/[A-Z][a-z]{2} \d+/);
    // Time part (HH:MM AM/PM or HH:MM)
    expect(out).toMatch(/\d{1,2}:\d{2}/);
  });
});

describe('fmtAddress (v0.94)', () => {
  it('returns em-dash for null / undefined', () => {
    expect(fmtAddress(null)).toBe('—');
    expect(fmtAddress(undefined)).toBe('—');
  });

  it('returns full address when shorter than head+tail+1', () => {
    expect(fmtAddress('0xabc')).toBe('0xabc');
  });

  it('truncates long addresses with head…tail', () => {
    const addr = '0x1234567890abcdef1234567890abcdef12345678';
    expect(fmtAddress(addr)).toBe('0x1234…5678');
  });
});
