// v0.94 — format.ts 边界用例测试（5 个测试,+~5 条语句）。
//
// 为 fmtRelativeTime、fmtDate、fmtDateTime、fmtAddress 添加测试
//（现有 formatRetentionAge-only 测试文件未覆盖的 4 个最常用
// 格式化器）。每个格式化器都有一个 `null`
// / `undefined` 分支以及若干数量级分支。

import { describe, it, expect } from 'vitest';
import {
  formatRetentionAge,
  fmtRelativeTime,
  fmtDate,
  fmtDateTime,
  fmtAddress,
  fmtUsdc,
  fmtPct,
  fmtPctInt,
  fmtEdge,
  fmtConfidence,
  fmtLatency,
  fmtCents,
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
    // 使用 Date.now() 避免时区漂移
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
    // 日期部分
    expect(out).toMatch(/[A-Z][a-z]{2} \d+/);
    // 时间部分(HH:MM AM/PM 或 HH:MM)
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

describe('numeric formatters (v0.95)', () => {
  it('fmtUsdc returns em-dash for null / undefined / non-finite', () => {
    expect(fmtUsdc(null)).toBe('—');
    expect(fmtUsdc(undefined)).toBe('—');
    expect(fmtUsdc('not a number')).toBe('—');
  });

  it('fmtUsdc formats number with 2 decimal places', () => {
    expect(fmtUsdc(123.45)).toBe('123.45');
    expect(fmtUsdc('100')).toBe('100.00');
  });

  it('fmtPct returns em-dash for null / undefined / non-finite', () => {
    expect(fmtPct(null)).toBe('—');
    expect(fmtPct(undefined)).toBe('—');
    expect(fmtPct(Infinity)).toBe('—');
  });

  it('fmtPct formats 0.15 as "15.0%"', () => {
    expect(fmtPct(0.15)).toBe('15.0%');
  });

  it('fmtPct signed=true adds + prefix for positive', () => {
    expect(fmtPct(0.15, true)).toBe('+15.0%');
  });

  it('fmtPct signed=true omits + for negative', () => {
    expect(fmtPct(-0.15, true)).toBe('-15.0%');
  });

  it('fmtPctInt formats 0.5 as "50%" (no decimal)', () => {
    expect(fmtPctInt(0.5)).toBe('50%');
  });

  it('fmtPctInt returns em-dash for null', () => {
    expect(fmtPctInt(null)).toBe('—');
  });

  it('fmtEdge always has +/- prefix', () => {
    expect(fmtEdge(0.15)).toBe('+15.0%');
    expect(fmtEdge(-0.15)).toBe('-15.0%');
  });

  it('fmtEdge returns em-dash for null', () => {
    expect(fmtEdge(null)).toBe('—');
  });

  it('fmtConfidence returns em-dash for null', () => {
    expect(fmtConfidence(null)).toBe('—');
  });

  it('fmtConfidence formats 0.85 as "85.0%"', () => {
    expect(fmtConfidence(0.85)).toBe('85.0%');
  });

  it('fmtLatency formats < 1000 as ms', () => {
    expect(fmtLatency(123)).toBe('123ms');
  });

  it('fmtLatency formats >= 1000 as seconds with 1 decimal', () => {
    expect(fmtLatency(1500)).toBe('1.5s');
  });

  it('fmtLatency returns em-dash for null', () => {
    expect(fmtLatency(null)).toBe('—');
  });

  it('fmtCents converts cents to dollars (100 → $1.00)', () => {
    expect(fmtCents(100)).toBe('$1.00');
  });

  it('fmtCents returns em-dash for null', () => {
    expect(fmtCents(null)).toBe('—');
  });
});
