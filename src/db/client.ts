import Database from 'better-sqlite3';
import { drizzle } from 'drizzle-orm/better-sqlite3';
import * as schema from './schema';
import { appDataDir, join } from '@tauri-apps/api/path';
import { existsSync, mkdirSync } from 'node:fs';

let _db: ReturnType<typeof drizzle<typeof schema>> | null = null;

const DB_FILENAME = 'polyrocket.db';

/**
 * 解析 Tauri app data 目录下的 DB 路径。
 * 在浏览器开发环境下回退到 ~/.polyrocket/polyrocket.db。
 */
async function resolveDbPath(): Promise<string> {
  try {
    const dir = await appDataDir();
    if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
    return await join(dir, DB_FILENAME);
  } catch {
    // 不在 Tauri 上下文中（在纯浏览器里跑 vite dev）
    const fallback = `${process.env.HOME}/.polyrocket`;
    if (!existsSync(fallback)) mkdirSync(fallback, { recursive: true });
    return `${fallback}/${DB_FILENAME}`;
  }
}

export async function getDb() {
  if (_db) return _db;
  const path = await resolveDbPath();
  const sqlite = new Database(path);
  // pragma 设置
  sqlite.pragma('journal_mode = WAL');
  sqlite.pragma('synchronous = NORMAL');
  sqlite.pragma('foreign_keys = ON');
  _db = drizzle(sqlite, { schema });
  return _db;
}

export { schema };