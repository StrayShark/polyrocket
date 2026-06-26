// v0.58c —— 验证 3 主题的对比度审计
// 通过 WCAG AA。我们通过脚本（Node）
// 子进程执行并断言其退出码 + summary 行。
// 脚本本身负责数学计算（相对亮度、
// 对比度）。

import { describe, it, expect } from 'vitest';
import { execSync } from 'node:child_process';

describe('3 主题 WCAG AA 对比度（v0.58c）', () => {
  it('check-theme-contrast.mjs 退出码为 0（所有对比度 >= 3.0:1）', () => {
    let out = '';
    let code = 0;
    try {
      out = execSync('node scripts/check-theme-contrast.mjs', {
        encoding: 'utf8',
      });
    } catch (e: unknown) {
      const err = e as { status?: number; stdout?: Buffer | string; stderr?: Buffer | string };
      code = err.status ?? 1;
      out = String(err.stdout ?? '') + String(err.stderr ?? '');
    }
    if (code !== 0) {
      // eslint-disable-next-line no-console
      console.error('check-theme-contrast output:\n' + out);
    }
    expect(code).toBe(0);
    expect(out).toMatch(/Summary:\s*✓ PASS/);
  });

  it('每个主题针对每个前景 token 至少有 1 个 PASS', () => {
    const out = execSync('node scripts/check-theme-contrast.mjs', {
      encoding: 'utf8',
    });
    // 期望有 3 个 === 块（每个主题一个）。
    const themeBlocks = out.match(/=== \w+ ===/g) ?? [];
    expect(themeBlocks.length).toBe(3);
  });

  it('任何主题都没有 FAIL 行（强制 >= 3.0:1）', () => {
    const out = execSync('node scripts/check-theme-contrast.mjs', {
      encoding: 'utf8',
    });
    // 脚本对低于 3.0:1 的对比度
    // 输出 `FAIL`。一个干净的审计
    // 不应该出现任何 FAIL 行。
    const failRows = (out.match(/FAIL/g) ?? []).length;
    expect(failRows).toBe(0);
  });
});
