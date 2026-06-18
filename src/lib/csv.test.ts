// v0.62a — csv lib tests.
//
// `toCsv` and `downloadCsv` are used by the
// LLM stats export. Today 0% coverage.
// This file covers:
//   1. toCsv: empty array → empty string
//   2. toCsv: simple rows → header + body
//   3. toCsv: quoted cells (commas, quotes, newlines)
//   4. toCsv: custom column order
//   5. downloadCsv: triggers a download (blob URL)

// @vitest-environment happy-dom

import { describe, it, expect, vi } from 'vitest';
import { toCsv, downloadCsv } from './csv';

describe('toCsv', () => {
  it('returns empty string for empty rows', () => {
    expect(toCsv([])).toBe('');
  });

  it('converts simple rows to header + body', () => {
    const csv = toCsv([
      { a: 1, b: 2 },
      { a: 3, b: 4 },
    ]);
    const lines = csv.trim().split('\n');
    expect(lines[0]).toBe('a,b');
    expect(lines[1]).toBe('1,2');
    expect(lines[2]).toBe('3,4');
  });

  it('quotes cells containing commas', () => {
    const csv = toCsv([{ a: 'x,y', b: 'z' }]);
    expect(csv.trim()).toBe('a,b\n"x,y",z');
  });

  it('escapes embedded double quotes by doubling them', () => {
    const csv = toCsv([{ a: 'he said "hi"' }]);
    expect(csv.trim()).toBe('a\n"he said ""hi"""');
  });

  it('respects a custom column order', () => {
    const csv = toCsv(
      [{ a: 1, b: 2, c: 3 }],
      ['c', 'a', 'b'],
    );
    expect(csv.trim().split('\n')[0]).toBe('c,a,b');
  });

  it('coerces null/undefined cells to empty', () => {
    const csv = toCsv([{ a: null, b: undefined, c: 'x' }]);
    expect(csv.trim()).toBe('a,b,c\n,,x');
  });
});

describe('downloadCsv', () => {
  it('creates a blob URL and clicks an anchor', () => {
    const createUrl = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:fake');
    const revokeUrl = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => {});
    const clickSpy = vi.fn();
    const origCreate = document.createElement.bind(document);
    const createSpy = vi.spyOn(document, 'createElement').mockImplementation((tag: string) => {
      const el = origCreate(tag) as HTMLAnchorElement;
      if (tag === 'a') {
        el.click = clickSpy;
      }
      return el;
    });

    downloadCsv('test.csv', 'a,b\n1,2');

    expect(createUrl).toHaveBeenCalled();
    expect(clickSpy).toHaveBeenCalled();
    expect(revokeUrl).toHaveBeenCalled();

    createSpy.mockRestore();
    createUrl.mockRestore();
    revokeUrl.mockRestore();
  });
});
