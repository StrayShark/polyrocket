#!/usr/bin/env node
// polyrocket — LLM 模型版本更新检查器 (v0.112)。
//
// 用途: §15.7 当前最新版本表 vs 5 家国产厂商 (Qwen / Doubao / Kimi / GLM / MiniMax /
//   ERNIE) 官方模型列表的 diff 检测。Cron 14 天跑一次,发现新 stable model ID
//   时 exit code 1 + 输出 diff 报告。Mavis cron prompt 看到 exit 1 就提议开 PR。
//
// 设计原则:
//   1. **只读,不写**: 不直接改 LlmStep.tsx,只输出 diff 给 Mavis 决策。
//   2. **fail-soft**: 任一厂商 fetch 失败 → 跳过该厂商,其它继续,exit 0 (unless
//      至少一个厂商有 diff)。
//   3. **Regex-based extraction**: 不做完整 HTML 解析,5 家厂商页面结构差异大,
//      用 provider-specific regex 抓取 model IDs 即可 (够 v0.112 用,后续可
//      换成 cheerio / readability)。
//   4. **CI-friendly**: 不依赖外网也能跑 (--offline 模式用 fixture)。
//
// 关联:
//   - docs/polyrocket-llm-management.md §15.7 — 当前最新版本表 (the source of truth)
//   - docs/llm-providers.md §1 — 11-provider matrix
//   - src/components/welcome/LlmStep.tsx — PROVIDERS 数组的 defaultModel 字段
//
// 退出码:
//   0 = 无 diff (或仅 fetch 失败,无 diff)
//   1 = 至少一个厂商有 new stable model ID
//   2 = 参数错误 / 文件缺失

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(__dirname, '..');
const SECTION_15_7_PATH = join(REPO_ROOT, 'docs/polyrocket-llm-management.md');

// v0.112 — 6 个国产 provider 的厂商文档 URL + 抓取 regex 模板。
// **注意**: regex 只抓 "<...>-<ver>" 形式的 model ID (e.g. `qwen3.7-max` /
//   `glm-5.2` / `ernie-5.0`),不抓 legacy 型号 / preview / fine-tune。
//
// 各家厂商页面结构差异大,regex 是 best-effort。后续 v0.112.x 可换成
// cheerio + 厂商-specific HTML 解析。
const PROVIDERS = [
  {
    id: 'qwen',
    name: '通义千问',
    url: 'https://help.aliyun.com/zh/model-studio/getting-started/models',
    // 匹配: qwen3.7-max / qwen3.6-flash / qwen2.5-max-preview 等
    // 排除: qwen-vl-max (多模态, 不在 default_model 范围)
    regex: /qwen(\d+[\.\-]\d+)[\-_]([a-z0-9\-]+)/gi,
    versionFromMatch: (m) => `qwen${m[1]}-${m[2]}`,
  },
  {
    id: 'doubao',
    name: '豆包',
    url: 'https://www.volcengine.com/docs/82379',
    // 匹配: doubao-seed-2-0-pro-260215 / doubao-1-5-pro-32k 等
    regex: /doubao[\-_]([a-z0-9\-]+)/gi,
    versionFromMatch: (m) => `doubao-${m[1]}`,
  },
  {
    id: 'kimi',
    name: 'Kimi',
    url: 'https://platform.moonshot.cn/docs/intro',
    // 匹配: kimi-k2.7-code / kimi-k2-0711-preview 等
    regex: /kimi[\-_](k?[\d\.]+[\-_][a-z0-9\-]+)/gi,
    versionFromMatch: (m) => `kimi-${m[1]}`,
  },
  {
    id: 'glm',
    name: '智谱 GLM',
    url: 'https://open.bigmodel.cn/cn/guide/start/model-overview',
    // 匹配: glm-5.2 / glm-4-plus / glm-4-9b 等
    regex: /glm[\-_]([\d\.]+[\-_a-z0-9]*)/gi,
    versionFromMatch: (m) => `glm-${m[1]}`,
  },
  {
    id: 'MiniMax',
    name: 'MiniMax',
    url: 'https://api.minimax.chat/document',
    // 匹配: MiniMax-M2.7 / MiniMax-Text-01 / abab-7 等
    regex: /MiniMax[\-_]([A-Za-z][\d\.]+[\-_a-zA-Z0-9]*)/gi,
    versionFromMatch: (m) => `MiniMax-${m[1]}`,
  },
  {
    id: 'ernie',
    name: '文心一言 ERNIE',
    url: 'https://cloud.baidu.com/doc/WENXINWORKSHOP/s/hk7k8k4ru',
    // 匹配: ernie-5.0 / ernie-4.5 / ernie-3.5-8k 等
    regex: /ernie[\-_]([\d\.]+[\-_a-z0-9]*)/gi,
    versionFromMatch: (m) => `ernie-${m[1]}`,
  },
];

// Parse §15.7 table — extract `default_model` (4th col) per provider row.
function parseCurrentModels(mdText) {
  // §15.7 table looks like:
  //   | Provider | `default_model` (代码当前值) | 厂商文档 | 调研日期 | 备注 |
  //   |---|---|---|---|---|
  //   | Qwen (通义千问) | `qwen3.7-max` | [Alibaba...] | 2026-06-22 | ... |
  //   ...
  // We extract: provider_name + `default_model` value
  const out = {};
  const lines = mdText.split('\n');
  let inSection = false;
  for (const line of lines) {
    if (line.startsWith('### 15.7')) inSection = true;
    else if (inSection && line.startsWith('### ')) break;
    if (!inSection) continue;
    // Match row: | provider | `model` | ...
    const m = line.match(/^\|\s*([^|]+?)\s*\|\s*`([^`]+)`\s*\|/);
    if (!m) continue;
    const providerLabel = m[1].trim();
    const model = m[2].trim();
    // Find PROVIDERS entry that matches this label
    for (const p of PROVIDERS) {
      if (providerLabel.toLowerCase().includes(p.name.toLowerCase()) ||
          providerLabel.toLowerCase().includes(p.id.toLowerCase())) {
        out[p.id] = { current: model, fetched: null, candidates: [] };
        break;
      }
    }
  }
  return out;
}

// Fetch URL — supports --offline mode (uses fixture for tests).
async function fetchUrl(url, { offline = false, fixture = null } = {}) {
  if (offline) {
    if (!fixture) throw new Error('offline mode requires fixture');
    return fixture;
  }
  const res = await fetch(url, {
    headers: { 'User-Agent': 'polyrocket-llm-checker/0.112' },
    signal: AbortSignal.timeout(10_000),
  });
  if (!res.ok) {
    throw new Error(`HTTP ${res.status} fetching ${url}`);
  }
  return res.text();
}

// Extract model candidates from page HTML using provider's regex.
function extractCandidates(html, provider) {
  const seen = new Set();
  const out = [];
  let m;
  // Reset regex state (g flag requires this)
  provider.regex.lastIndex = 0;
  while ((m = provider.regex.exec(html)) !== null) {
    const id = provider.versionFromMatch(m);
    if (seen.has(id)) continue;
    seen.add(id);
    out.push(id);
  }
  return out;
}

// Filter candidates to "stable" (no preview / experimental / turbo).
function isStableModelId(id) {
  const lower = id.toLowerCase();
  if (lower.includes('preview')) return false;
  if (lower.includes('experimental')) return false;
  if (lower.includes('exp-')) return false;
  if (lower.includes('-internal')) return false;
  // Numbers in id look like "ernie-5.0" → minor version 0+ allowed
  return true;
}

// Pick the "newest stable" candidate.
// Compare by parsing version segments (e.g. "3.7" → [3, 7]) and ordering
// each segment numerically. This is a heuristic — real SemVer would be
// better but overkill for the 5-6 vendors we track.
function pickNewest(candidates) {
  const stable = candidates.filter(isStableModelId);
  if (stable.length === 0) return null;
  // Split candidate into [prefix, version-digits-array].
  // E.g. "qwen3.7-max" → ["qwen", [3, 7], "max"]
  function versionParts(id) {
    // Find all digit groups (with optional dots) anywhere in the string.
    // Treat any single dot-separated digit run as a version segment.
    const matches = id.match(/\d+(?:\.\d+)*/g) || [];
    return matches.flatMap((s) => s.split('.').map((x) => parseInt(x, 10)));
  }
  return stable.sort((a, b) => {
    const av = versionParts(a);
    const bv = versionParts(b);
    const len = Math.max(av.length, bv.length);
    for (let i = 0; i < len; i++) {
      const ax = av[i] || 0;
      const bx = bv[i] || 0;
      if (ax !== bx) return bx - ax;
    }
    // Tiebreaker: longer string first (more specific)
    return b.length - a.length;
  })[0];
}

async function main() {
  const args = process.argv.slice(2);
  const offline = args.includes('--offline');
  const json = args.includes('--json');

  // 1. Parse current models from §15.7
  const mdText = readFileSync(SECTION_15_7_PATH, 'utf-8');
  const current = parseCurrentModels(mdText);

  // 2. Fetch + extract for each provider
  const results = {};
  for (const p of PROVIDERS) {
    if (!current[p.id]) {
      results[p.id] = { error: 'not in §15.7 table', provider: p.name };
      continue;
    }
    let html;
    try {
      const fixtureIdx = args.indexOf(`--fixture-${p.id}`);
      const fixture = fixtureIdx >= 0 ? args[fixtureIdx + 1] : null;
      html = await fetchUrl(p.url, { offline, fixture });
    } catch (e) {
      results[p.id] = { error: e.message, provider: p.name, url: p.url };
      continue;
    }
    const candidates = extractCandidates(html, p);
    const newest = pickNewest(candidates);
    results[p.id] = {
      provider: p.name,
      url: p.url,
      current: current[p.id].current,
      fetched: candidates.length > 0,
      candidateCount: candidates.length,
      candidates: candidates.slice(0, 10),
      newest,
      diff: newest && newest !== current[p.id].current,
    };
  }

  // 3. Output
  const hasDiff = Object.values(results).some((r) => r.diff);
  if (json) {
    console.log(JSON.stringify({ hasDiff, results }, null, 2));
  } else {
    console.log(`# LLM model version check (v0.112)\n`);
    console.log(`§15.7 current vs vendor fetch:\n`);
    for (const [id, r] of Object.entries(results)) {
      if (r.error) {
        console.log(`  ${r.provider.padEnd(16)} ❌ ${r.error}`);
        continue;
      }
      const marker = r.diff ? '🆕' : '✅';
      console.log(`  ${r.provider.padEnd(16)} ${marker} current: ${r.current.padEnd(28)} | newest: ${r.newest || '(none)'}`);
    }
    console.log('');
    if (hasDiff) {
      console.log(`❗ At least one provider has a new stable model.`);
      console.log(`   Mavis should propose a PR updating §15.7 + LlmStep.tsx + llm-providers.md §1.`);
    } else {
      console.log(`✅ All providers up to date. No PR needed.`);
    }
  }

  process.exit(hasDiff ? 1 : 0);
}

main().catch((e) => {
  console.error(`FATAL: ${e.message}`);
  process.exit(2);
});
