import Database from 'better-sqlite3';
import { drizzle } from 'drizzle-orm/better-sqlite3';
import * as schema from './schema';
import { appDataDir, join } from '@tauri-apps/api/path';
import { existsSync, mkdirSync } from 'node:fs';

let _db: ReturnType<typeof drizzle<typeof schema>> | null = null;

const DB_FILENAME = 'polyrocket.db';

/**
 * Resolve DB path inside Tauri app data dir.
 * Falls back to ~/.polyrocket/polyrocket.db in browser dev.
 */
async function resolveDbPath(): Promise<string> {
  try {
    const dir = await appDataDir();
    if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
    return await join(dir, DB_FILENAME);
  } catch {
    // not in Tauri context (vite dev in plain browser)
    const fallback = `${process.env.HOME}/.polyrocket`;
    if (!existsSync(fallback)) mkdirSync(fallback, { recursive: true });
    return `${fallback}/${DB_FILENAME}`;
  }
}

export async function getDb() {
  if (_db) return _db;
  const path = await resolveDbPath();
  const sqlite = new Database(path);
  // pragmas
  sqlite.pragma('journal_mode = WAL');
  sqlite.pragma('synchronous = NORMAL');
  sqlite.pragma('foreign_keys = ON');
  _db = drizzle(sqlite, { schema });
  return _db;
}

export { schema };