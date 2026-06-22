// polyrocket — LLM 模型版本更新检查器 (v0.112) tests.
//
// Standalone Node test (uses node:test). 不需要外网,纯逻辑测试。
// Run: `node --test scripts/check-llm-updates.test.mjs`

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const exec = promisify(execFile);
const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, '..');
const SCRIPT = join(REPO_ROOT, 'scripts', 'check-llm-updates.mjs');

const QWEN_FIXTURE_NEWER = `
<html><body>
  <h1>Qwen Models</h1>
  <ul>
    <li>qwen3.7-max (latest)</li>
    <li>qwen3.7-plus</li>
    <li>qwen3.6-flash</li>
    <li>qwen-vl-max (vision, not in default)</li>
  </ul>
</body></html>
`;

const QWEN_FIXTURE_SAME = `
<html><body>
  <h1>Qwen Models</h1>
  <ul>
    <li>qwen3.7-max</li>
    <li>qwen3.6-flash</li>
  </ul>
</body></html>
`;

const QWEN_FIXTURE_PREVIEW_ONLY = `
<html><body>
  <ul>
    <li>qwen3.7-max-preview</li>
    <li>qwen3.8-experimental</li>
  </ul>
</body></html>
`;

describe('check-llm-updates.mjs', () => {
  describe('§15.7 table parsing', () => {
    it('script runs and returns JSON with results object', async () => {
      const { stdout } = await exec('node', [SCRIPT, '--offline', '--json', '--fixture-qwen', QWEN_FIXTURE_SAME]);
      const out = JSON.parse(stdout);
      assert.ok('results' in out, 'should have results key');
      assert.ok('qwen' in out.results, 'should have qwen provider result');
    });

    it('parses 6 providers from §15.7 table (Qwen + Doubao + Kimi + GLM + MiniMax + ERNIE)', async () => {
      const { stdout } = await exec('node', [SCRIPT, '--offline', '--json', '--fixture-qwen', QWEN_FIXTURE_SAME]);
      const out = JSON.parse(stdout);
      const expectedIds = ['qwen', 'doubao', 'kimi', 'glm', 'MiniMax', 'ernie'];
      for (const id of expectedIds) {
        assert.ok(id in out.results, `should have ${id} provider result`);
      }
    });
  });

  describe('diff detection', () => {
    it('no diff when fixture has same model as current', async () => {
      const { stdout } = await exec('node', [SCRIPT, '--offline', '--json', '--fixture-qwen', QWEN_FIXTURE_SAME]);
      const out = JSON.parse(stdout);
      assert.equal(out.results.qwen.diff, false);
      assert.equal(out.results.qwen.newest, 'qwen3.7-max');
      assert.equal(out.hasDiff, false);
    });

    it('detects diff when fixture has newer model (qwen3.8-flash)', async () => {
      const newer = `<li>qwen3.8-flash</li><li>qwen3.7-max</li>`;
      let caught = null;
      try {
        await exec('node', [SCRIPT, '--offline', '--json', '--fixture-qwen', newer]);
      } catch (e) {
        caught = e;
      }
      assert.ok(caught !== null, 'should exit 1 on diff');
      assert.equal(caught.code, 1);
      const out = JSON.parse(caught.stdout);
      assert.equal(out.results.qwen.diff, true);
      assert.equal(out.results.qwen.newest, 'qwen3.8-flash');
      assert.equal(out.hasDiff, true);
    });
  });

  describe('stable filter', () => {
    it('excludes preview / experimental candidates', async () => {
      const { stdout } = await exec('node', [SCRIPT, '--offline', '--json', '--fixture-qwen', QWEN_FIXTURE_PREVIEW_ONLY]);
      const out = JSON.parse(stdout);
      // No stable candidates → newest is null → no diff (diff is null because newest is null)
      assert.equal(out.results.qwen.newest, null);
      assert.notEqual(out.results.qwen.diff, true);
    });
  });

  describe('newest pick (lexicographic version compare)', () => {
    it('picks highest version segment', async () => {
      const mixed = `<li>qwen3.5-max</li><li>qwen3.7-max</li><li>qwen3.6-flash</li>`;
      const { stdout } = await exec('node', [SCRIPT, '--offline', '--json', '--fixture-qwen', mixed]);
      const out = JSON.parse(stdout);
      // qwen3.7-max (3.7) > qwen3.6-flash (3.6) > qwen3.5-max (3.5)
      assert.equal(out.results.qwen.newest, 'qwen3.7-max');
    });
  });

  describe('exit codes', () => {
    it('exit 0 when no diff', async () => {
      const { stdout } = await exec('node', [SCRIPT, '--offline', '--json', '--fixture-qwen', QWEN_FIXTURE_SAME]);
      // If we get here, exit was 0
      assert.ok(stdout);
    });

    it('exit 1 when diff detected', async () => {
      let caught = null;
      try {
        await exec('node', [SCRIPT, '--offline', '--json', '--fixture-qwen', `<li>qwen4.0-max</li>`]);
      } catch (e) {
        caught = e;
      }
      assert.ok(caught !== null, 'should have thrown on exit 1');
      assert.equal(caught.code, 1);
    });
  });
});
