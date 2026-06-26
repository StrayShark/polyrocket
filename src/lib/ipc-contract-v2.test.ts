// polyrocket —— L1 IPC 契约 v2 测试（v0.67b）。
//
// 在 v0.66c 基于快照的检查之上扩展，覆盖更深：
//
// v0.66c 检查：
//   - src/ipc.ts 中函数的新增/移除
//   - 参数数量变化
//
// v0.67b 新增：
//   - L1 包装器使用的类型导入（如 `Wallet`、`Bet`）
//   - 每个包装器的返回类型注解
//
// **为何不做完整的 ts-rs/specta codegen**：真正的 codegen
// 配置需要：
//   1. 在 Cargo.toml 添加 `specta`、`specta-typescript`、`tauri-specta`
//   2. 给所有 `#[tauri::command]` 函数标注 `#[specta]`
//   3. 运行 `cargo run --bin specta-gen` 生成 TS 类型
//   4. 用生成的包装器替换 src/ipc.ts
// 预计 3 小时以上且存在破坏现有 IPC 层的风险。
// v0.67b 这个桩测试用 1/20 的时间捕获相同的漂移情形
// （函数名 + 参数数量 + 返回类型 + DTO 引用）。
//
// **测试失败时**：有人修改了 IPC 包装器的
// 返回类型、为 DTO 添加了字段，或新增/移除了
// 类型导入。修复方法是同时更新两层
// （src/ipc.ts 和 src-tauri/src/commands/*.rs）。
// 参见 docs/overview.md §1.2。

import { describe, it, expect } from 'vitest';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const SRC_DIR = join(import.meta.dirname, '..');
const IPC_PATH = join(SRC_DIR, 'ipc.ts');
const SNAPSHOT_PATH = join(SRC_DIR, 'ipc.snapshot.v2.json');

interface IpcSnapshotV2 {
  functions: Record<string, {
    params: number;
    /** 参数名（尽力而为）。 */
    paramNames: string[];
/** 源码中编写的返回类型(字符串)。 */
    returnType: string | null;
  }>;
  /** ipc.ts 引用的类型导入。 */
  typeImports: string[];
  total: number;
}

/** 匹配 `export const NAME = (...args...): ReturnType => ...` */
const ARROW_RE = /^export const (\w+)\s*=\s*(?:async\s*)?\(([^)]*)\)(?:\s*:\s*([^\s=>{]+))?\s*=>/gm;
const TYPE_IMPORT_RE = /import\s+type\s*\{([^}]+)\}\s+from\s+['"]@\/types\/(\w+)['"]/g;

function parseIpcFile(): IpcSnapshotV2 {
  const src = readFileSync(IPC_PATH, 'utf8');
  const functions: IpcSnapshotV2['functions'] = {};
  let m: RegExpExecArray | null;

  while ((m = ARROW_RE.exec(src)) !== null) {
    const name = m[1];
    const paramsStr = m[2] ?? '';
    const returnType = m[3] ?? null;
    if (name) {
      const params = paramsStr.split(',').filter((p) => p.trim().length > 0);
      const paramNames = params.map((p) => {
        // 剥离默认值、类型注解和解构。
        // 我们只想要裸标识符。
        const m = p.match(/^\s*(?:\{|\[)?\s*([a-zA-Z_$][\w$]*)/);
        return m?.[1] ?? p.trim();
      });
      functions[name] = { params: params.length, paramNames, returnType };
    }
  }

  const typeImports: string[] = [];
  while ((m = TYPE_IMPORT_RE.exec(src)) !== null) {
    const names = (m[1] ?? '').split(',').map((n) => n.trim()).filter(Boolean);
    typeImports.push(...names);
  }

  return { functions, typeImports: [...new Set(typeImports)].sort(), total: Object.keys(functions).length };
}

describe('L1 IPC contract v2 (v0.67b)', () => {
  it('matches the v2 snapshot (function names + params + return types + DTO imports)', () => {
    const current = parseIpcFile();

    // 健全性检查:至少 100 个包装器
    expect(current.total).toBeGreaterThanOrEqual(100);

    if (!existsSync(SNAPSHOT_PATH)) {
      writeFileSync(SNAPSHOT_PATH, JSON.stringify(current, null, 2) + '\n');
      return; // 首次运行
    }

    const snapshot: IpcSnapshotV2 = JSON.parse(readFileSync(SNAPSHOT_PATH, 'utf8'));
    const currentNames = Object.keys(current.functions).sort();
    const snapshotNames = Object.keys(snapshot.functions).sort();

    // 1. 函数漂移(同 v0.66c)
    const added = currentNames.filter((n) => !snapshotNames.includes(n));
    const removed = snapshotNames.filter((n) => !currentNames.includes(n));
    if (added.length > 0) {
      throw new Error(`IPC contract drift: ${added.length} new fn(s):\n` +
        added.map((n) => `  + ${n}`).join('\n') +
        `\nUpdate ${SNAPSHOT_PATH} if intentional.`);
    }
    if (removed.length > 0) {
      throw new Error(`IPC contract drift: ${removed.length} removed fn(s):\n` +
        removed.map((n) => `  - ${n}`).join('\n') +
        `\nUpdate ${SNAPSHOT_PATH} if intentional.`);
    }

    // 2. 参数漂移
    for (const name of currentNames) {
      const c = current.functions[name]!;
      const s = snapshot.functions[name]!;
      if (c.params !== s.params) {
        throw new Error(`IPC contract drift: '${name}' param count: ${s.params} → ${c.params}`);
      }
      // 参数名（尽力而为）
      for (let i = 0; i < c.paramNames.length; i++) {
        if (c.paramNames[i] !== s.paramNames[i]) {
          throw new Error(`IPC contract drift: '${name}' param ${i}: '${s.paramNames[i]}' → '${c.paramNames[i]}'`);
        }
      }
      // 返回类型漂移
      if (c.returnType !== s.returnType) {
        throw new Error(`IPC contract drift: '${name}' return type: '${s.returnType}' → '${c.returnType}'`);
      }
    }

    // 3. 类型导入漂移
    const currentImports = [...current.typeImports].sort();
    const snapshotImports = [...snapshot.typeImports].sort();
    const addedTypes = currentImports.filter((t) => !snapshotImports.includes(t));
    const removedTypes = snapshotImports.filter((t) => !currentImports.includes(t));
    if (addedTypes.length > 0) {
      throw new Error(`IPC contract drift: ${addedTypes.length} new DTO type(s) imported:\n` +
        addedTypes.map((t) => `  + ${t}`).join('\n') +
        `\nUpdate ${SNAPSHOT_PATH} if intentional.`);
    }
    if (removedTypes.length > 0) {
      throw new Error(`IPC contract drift: ${removedTypes.length} DTO type(s) removed:\n` +
        removedTypes.map((t) => `  - ${t}`).join('\n') +
        `\nUpdate ${SNAPSHOT_PATH} if intentional.`);
    }
  });
});
