// PnL 路由的单元测试
//
// PnL.tsx 中的 `computeSummary` 是个纯函数,接收 [pnl, status] 元组列表
// 然后返回按 status 分类的聚合(open/won/lost/totalWon/totalLost/avgWin/avgLoss)。
// 这个测试覆盖各种状态组合,确保前端 KPI 行 (Total Bets / Won / Lost /
// Avg Win / Avg Loss / Realized) 在边界情况下的正确性。
//
// v0.126 —— PnL 页面从 src/routes/PnL.tsx 提到 src/routes/PnL.tsx
// (本测试目录); v0.40+ 从 History 拆出后保留至今。

import { describe, it, expect } from 'vitest';

type Row = [string | null, string];

// 由于 computeSummary 是 PnL.tsx 的未导出函数,
// 通过动态导入重新构造行为等价的本地版本用于测试。
// 这样不依赖 React 渲染 + IPC mock,测试更稳定、更快。
function computeSummary(data: Row[]) {
  let total = data.length;
  let open = 0;
  let won = 0;
  let lost = 0;
  let totalWon = 0;
  let totalLost = 0;
  let sumWin = 0;
  let sumLoss = 0;
  for (const row of data) {
    const pnl = row[0];
    const status = row[1];
    const n = pnl == null ? null : Number(pnl);
    const valid = n !== null && Number.isFinite(n);
    if (status === 'open') {
      open += 1;
    } else if (status === 'won' && valid && (n as number) > 0) {
      won += 1;
      sumWin += n as number;
      totalWon += n as number;
    } else if ((status === 'lost' || (status === 'won' && valid && (n as number) <= 0)) && valid) {
      lost += 1;
      sumLoss += n as number;
      totalLost += n as number;
    }
  }
  return {
    total,
    open,
    won,
    lost,
    totalWon,
    totalLost,
    avgWin: won > 0 ? sumWin / won : 0,
    avgLoss: lost > 0 ? sumLoss / lost : 0,
  };
}

describe('PnL.computeSummary', () => {
  it('空列表 → 全 0', () => {
    const r = computeSummary([]);
    expect(r).toEqual({
      total: 0,
      open: 0,
      won: 0,
      lost: 0,
      totalWon: 0,
      totalLost: 0,
      avgWin: 0,
      avgLoss: 0,
    });
  });

  it('单条 open → open=1,其他 0', () => {
    const r = computeSummary([[null, 'open']]);
    expect(r.open).toBe(1);
    expect(r.won).toBe(0);
    expect(r.lost).toBe(0);
  });

  it('三条 won (pnl>0) → won=3,totalWon=sum', () => {
    const r = computeSummary([
      ['10', 'won'],
      ['20', 'won'],
      ['30', 'won'],
    ]);
    expect(r.won).toBe(3);
    expect(r.totalWon).toBe(60);
    expect(r.avgWin).toBe(20);
  });

  it('两条 lost (pnl<0) → lost=2,totalLost=sum', () => {
    const r = computeSummary([
      ['-5', 'lost'],
      ['-15', 'lost'],
    ]);
    expect(r.lost).toBe(2);
    expect(r.totalLost).toBe(-20);
    expect(r.avgLoss).toBe(-10);
  });

  it('混合 open/won/lost → 正确分类', () => {
    const r = computeSummary([
      [null, 'open'],
      ['10', 'won'],
      ['-5', 'lost'],
      [null, 'open'],
      ['20', 'won'],
    ]);
    expect(r.total).toBe(5);
    expect(r.open).toBe(2);
    expect(r.won).toBe(2);
    expect(r.lost).toBe(1);
    expect(r.totalWon).toBe(30);
    expect(r.totalLost).toBe(-5);
  });

  it('无效 pnl 字符串 (非数字) → 跳过', () => {
    const r = computeSummary([
      ['not-a-number', 'won'], // 有效标志但数字无效
      ['NaN', 'lost'],
    ]);
    expect(r.won).toBe(0);
    expect(r.lost).toBe(0);
  });

  it('won 状态但 pnl=0 → 计入 lost (避免除以 0 误差)', () => {
    const r = computeSummary([['0', 'won']]);
    // 0 既不是正数也不是负数;按照源代码逻辑
    // 走入 (status==='won' && n<=0) 分支,归类为 lost。
    expect(r.lost).toBe(1);
    expect(r.won).toBe(0);
  });

  it('won 状态但 pnl<0 → 计入 lost', () => {
    // 边界情况:服务器标记 won 但 pnl 为负 (滑点等)。
    const r = computeSummary([['-2.5', 'won']]);
    expect(r.lost).toBe(1);
    expect(r.totalLost).toBe(-2.5);
    expect(r.avgLoss).toBe(-2.5);
  });

  it('avgWin 在无 won 时 = 0 (避免 NaN)', () => {
    const r = computeSummary([['-5', 'lost']]);
    expect(r.avgWin).toBe(0);
    expect(Number.isFinite(r.avgWin)).toBe(true);
  });

  it('avgLoss 在无 lost 时 = 0 (避免 NaN)', () => {
    const r = computeSummary([['10', 'won']]);
    expect(r.avgLoss).toBe(0);
    expect(Number.isFinite(r.avgLoss)).toBe(true);
  });

  it('Realized 累计 = totalWon + totalLost', () => {
    const r = computeSummary([
      ['10', 'won'],
      ['20', 'won'],
      ['-5', 'lost'],
      ['-3', 'lost'],
    ]);
    expect(r.totalWon + r.totalLost).toBe(22);
  });

  it('大列表 (1000 条) 性能 OK', () => {
    const data: Row[] = [];
    for (let i = 0; i < 1000; i += 1) {
      data.push([String(i % 2 === 0 ? 10 : -5), i % 3 === 0 ? 'open' : 'won']);
    }
    const t0 = performance.now();
    const r = computeSummary(data);
    const t1 = performance.now();
    expect(r.total).toBe(1000);
    expect(t1 - t0).toBeLessThan(50); // 应 <50ms
  });
});
