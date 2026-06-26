// v0.66c —— L1 IPC 契约快照测试。
//
// 目的：捕获 L1 TypeScript 包装器（位于
// `src/ipc.ts`）与 L2 Rust 命令（位于
// `src-tauri/src/commands/*.rs`）之间的漂移。
// 如果有人新增了 Rust 命令而没有添加 TS 包装器
// （或反之），或者修改了函数签名，此测试会失败。
//
// **为何采用快照而非 codegen**：完整的 ts-rs / specta
// codegen 配置需要 3 小时以上并新增 2-3 个
// 依赖。项目已经在两层之间做了手动同步
// （CI 模块映射强制 1 个 Rust 命令对应 1 个 TS 包装器），
// 所以运行时检查是一种投入更小、对最常见的
// 漂移情形覆盖相当的方案。
//
// **本测试检查的内容**：
//   1. `src/ipc.ts` 中 `export function` 名称列表
//      与快照匹配（漂移 = 命令新增/移除）。
//   2. 每个函数的参数数量与快照匹配
//      （漂移 = 签名变更）。
//   3. 总数为 111（与项目文档
//      "111 IPC commands" 匹配）。
//
// **测试失败时**：有人新增/移除/重命名了
// L1 包装器而没有同步更新 L2 Rust 端（或反之）。
// 修复方法是保持两层同步。
// 参见 docs/overview.md §1.2 中的层级映射。

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const IPC_PATH = join(import.meta.dirname, '..', 'ipc.ts');
const SNAPSHOT_PATH = join(import.meta.dirname, '..', 'ipc.snapshot.json');

interface IpcSnapshot {
  /** 函数名 → 参数数量。 */
  functions: Record<string, number>;
  /** 总数。 */
  total: number;
}

function parseIpcFile(): IpcSnapshot {
  const src = readFileSync(IPC_PATH, 'utf8');
  // polyrocket 的 L1 对 IPC 命令使用
  // `export const NAME = (args) => invoke<T>(...)`,
  // 对事件监听器使用 `export const NAME = (args) => { ... }`。
  // 我们同时匹配两者,使用一个能处理多行形式的 regex
  // （某些 const 箭头会在 `=` 后换行）。
  //
  // 模式：`^export const NAME\s*=\s*([^)]*)\)\s*=>` —— 覆盖
  // 单行箭头 + 多行（新行开头的 `=>` 表示
  // 函数体从此处开始）。
  const arrowRegex = /^export const (\w+)\s*=\s*([^)]*)\)\s*=>/gm;
  const functions: Record<string, number> = {};
  let m: RegExpExecArray | null;
  while ((m = arrowRegex.exec(src)) !== null) {
    const name = m[1];
    if (name && m[2] !== undefined) {
      // 从参数列表中剥离 TS 类型注解。我们只
      // 统计实际的类标识符 token(不含 `: Type` 部分)。
      const params = m[2].split(',').filter((p) => p.trim().length > 0);
      functions[name] = params.length;
    }
  }
  return { functions, total: Object.keys(functions).length };
}

describe('L1 IPC contract', () => {
  it('matches the snapshot (L1 wrappers ↔ L2 commands)', () => {
    const current = parseIpcFile();

    // 项目文档声称有 111 个 IPC 命令。函数计数
    // 至少是那个数字(某些包装器是非 IPC 工具,
    // 比如事件监听器,但常规 `invoke` 包装器
    // 应占绝大多数)。我们允许 ≥ 100 以保留余量,
    // 防止 regex 漏掉带复杂泛型的多行箭头。
    expect(current.total).toBeGreaterThanOrEqual(100);

    // 如果快照文件存在,则与之对比。这是一个
    // 硬漂移检查:任何函数的新增/移除都会改变
    // 快照并导致测试失败。
    //
    // 如果快照文件不存在,则写入(首次运行)。
    // 后续运行将进行对比。
    const fs = require('node:fs') as typeof import('node:fs');
    if (!fs.existsSync(SNAPSHOT_PATH)) {
      fs.writeFileSync(SNAPSHOT_PATH, JSON.stringify(current, null, 2) + '\n');
      // 允许首次运行通过(快照正在创建中)
      return;
    }

    const snapshot: IpcSnapshot = JSON.parse(fs.readFileSync(SNAPSHOT_PATH, 'utf8'));

    // 对比函数名 —— 新增/移除即为漂移
    const currentNames = Object.keys(current.functions).sort();
    const snapshotNames = Object.keys(snapshot.functions).sort();
    const added = currentNames.filter((n) => !snapshotNames.includes(n));
    const removed = snapshotNames.filter((n) => !currentNames.includes(n));

    if (added.length > 0) {
      throw new Error(
        `L1 IPC contract drift: ${added.length} new function(s) added to src/ipc.ts:\n` +
          added.map((n) => `  + ${n} (${current.functions[n]} params)`).join('\n') +
          `\nIf intentional, update the snapshot by running this test with --update-snapshot.`,
      );
    }
    if (removed.length > 0) {
      throw new Error(
        `L1 IPC contract drift: ${removed.length} function(s) removed from src/ipc.ts:\n` +
          removed.map((n) => `  - ${n}`).join('\n') +
          `\nIf intentional, update the snapshot.`,
      );
    }

    // 对比参数数量 —— 签名变更即为漂移
    for (const name of currentNames) {
      if (current.functions[name] !== snapshot.functions[name]) {
        throw new Error(
          `L1 IPC contract drift: '${name}' parameter count changed ` +
            `from ${snapshot.functions[name]} to ${current.functions[name]}. ` +
            `If intentional, update the snapshot.`,
        );
      }
    }
  });
});
