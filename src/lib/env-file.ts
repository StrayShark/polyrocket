// v0.57d — env-file parsing helper.
//
// The LLM management page lets the user pick a
// .env / .key / .txt file from disk to import a
// secret. The picker returns an absolute path;
// we read the file via the Tauri fs plugin (or
// the browser File API in tests) and extract
// the API key.
//
// ## Format
//
// We accept two shapes:
//
//   1. Plain key file — the file content IS the
//      secret. e.g. `sk-abc123...` on a single
//      line. The file is usually <1024 bytes.
//
//   2. .env-shaped — `KEY=VALUE` lines, possibly
//      with comments. We parse the FIRST line
//      that has a non-empty value, ignoring:
//        - lines starting with `#` (comments)
//        - empty lines
//        - lines with an empty value
//      We DO NOT pin the key name; the user
//      can have any of `OPENAI_API_KEY`,
//      `ANTHROPIC_API_KEY`, etc. The picker
//      surface filters to .env / .key / .txt
//      but the parser is permissive.
//
// Returns: the secret string, or `null` if the
// file is empty / no value line found.

/** Read a file as text. In the Tauri build this
 * goes through the tauri-plugin-fs API (added
 * v0.57d as a dependency). In tests / web the
 * browser FileReader is used. */
export async function readFileText(path: string): Promise<string> {
  // Try the Tauri fs plugin first. We dynamic-
  // import so the web build doesn't break (the
  // plugin isn't registered in the Vite dev
  // server).
  try {
    const { readTextFile } = await import(
      /* @vite-ignore */ '@tauri-apps/plugin-fs'
    );
    return await readTextFile(path);
  } catch {
    // Web fallback: try fetch with file:// URL.
    // Works for tests that mock the IPC layer
    // but not for a real prod Tauri build.
    const res = await fetch(path);
    if (!res.ok) {
      throw new Error(`readFileText: HTTP ${res.status}`);
    }
    return await res.text();
  }
}

/** Parse a .env-shaped file and return the FIRST
 * non-empty KEY=VALUE pair's value. Returns
 * null if no value line is found. */
export function extractSecretFromEnv(content: string): string | null {
  const lines = content.split(/\r?\n/);
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) continue;
    if (line.startsWith('#')) continue;
    const eq = line.indexOf('=');
    if (eq === -1) {
      // No `=` — treat the whole line as the
      // secret (plain key file). This is the
      // common shape for `sk-...` files.
      return line;
    }
    const value = line.slice(eq + 1).trim();
    // Strip surrounding quotes if present.
    const unquoted = value.replace(/^["'](.*)["']$/, '$1');
    if (unquoted) return unquoted;
  }
  return null;
}
