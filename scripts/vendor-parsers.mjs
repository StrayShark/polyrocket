// polyrocket — per-vendor model ID extractors (v0.115).
//
// v0.112 用单一 regex 在脚本里 inline,5+ 家厂商页面结构差异大时混在一起
// 维护性差。v0.115 抽出 per-vendor `extractModels(html)` 函数,每个 vendor
// 一个文件,主脚本 import。
//
// **设计原则**:
//   1. 每个 vendor 一个 `extractModels(html): string[]` 函数
//   2. 返回候选 model IDs (含 "preview" / "lite" 等;主脚本会 filter stable)
//   3. 失败 → 返回 [] (fail-soft,跟 v0.112 一致)
//   4. 后续 v0.115.x 可逐个换 cheerio (v0.115 仍 regex,只为结构清晰)
//
// **不引 cheerio dep**: per CLAUDE.md / 现有约定,不动 package.json。
// 真正的 cheerio 升级 deferred v0.115.1+ (那时 cheerio 已是 transitive dep)。

/**
 * Generic fallback: 用最宽松 regex 找 vendor 名字后跟版本号。
 * 用于 vendor-specific 解析失败的 fallback。
 */
function genericExtract(html, vendorName) {
  const out = new Set();
  // 匹配 vendorName-VERSION (e.g. "qwen-3.7", "ernie-5.0")
  const escaped = vendorName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const re = new RegExp(`${escaped}[-_]?(\\d+(?:\\.\\d+)?[a-z\\d\\-]*)`, 'gi');
  let m;
  while ((m = re.exec(html)) !== null) {
    out.add(m[0].toLowerCase());
  }
  return Array.from(out);
}

/**
 * Aliyun Model Studio — 通义千问。
 * 页面结构: <code> 标签里 model name,或 <li> 里 "qwen<version>-<variant>"。
 * 例: qwen3.7-max, qwen3.7-plus, qwen3.6-flash
 */
export function extractQwen(html) {
  const out = new Set();
  // 排除 qwen-vl-* (多模态) 和 qwen-coder (coder 系列) — 这些不是 LLM default
  const re = /qwen(\d+[\.\-]\d+)(?:[\-_]([a-z0-9\-]+))?/gi;
  let m;
  while ((m = re.exec(html)) !== null) {
    if (m[0].includes('vl') || m[0].includes('coder')) continue;
    const version = m[1].replace('-', '.');
    const variant = m[2] || '';
    if (variant === 'vl' || variant === 'coder') continue;
    out.add(variant ? `qwen${version}-${variant}` : `qwen${version}`);
  }
  return Array.from(out);
}

/**
 * Volcengine 方舟 — 豆包。
 * 例: doubao-seed-2-0-pro-260215, doubao-1-5-pro-32k
 */
export function extractDoubao(html) {
  const out = new Set();
  const re = /doubao[\-_]([a-z0-9\-]+)/gi;
  let m;
  while ((m = re.exec(html)) !== null) {
    out.add(`doubao-${m[1].toLowerCase()}`);
  }
  return Array.from(out);
}

/**
 * Moonshot — Kimi。
 * 例: kimi-k2.7-code, kimi-k2-0711-preview
 */
export function extractKimi(html) {
  const out = new Set();
  const re = /kimi[\-_]([a-z0-9\.\-]+)/gi;
  let m;
  while ((m = re.exec(html)) !== null) {
    out.add(`kimi-${m[1].toLowerCase()}`);
  }
  return Array.from(out);
}

/**
 * 智谱 BigModel — GLM。
 * 例: glm-5.2, glm-4-plus, glm-4-flash
 */
export function extractGlm(html) {
  const out = new Set();
  const re = /glm[\-_]([\d\.]+[\-_a-z0-9]*)/gi;
  let m;
  while ((m = re.exec(html)) !== null) {
    out.add(`glm-${m[1].toLowerCase()}`);
  }
  return Array.from(out);
}

/**
 * MiniMax.
 * 例: MiniMax-M2.7, MiniMax-Text-01, abab-7
 */
export function extractMiniMax(html) {
  const out = new Set();
  // 包括两个命名空间:MiniMax-* 和 abab-*
  const re1 = /MiniMax[\-_]([A-Za-z][\d\.]+[\-_a-zA-Z0-9]*)/gi;
  let m;
  while ((m = re1.exec(html)) !== null) {
    out.add(`MiniMax-${m[1]}`);
  }
  // abab 系列 (MiniMax 旧命名) — 也采集
  const re2 = /\babab[\-_]?(\d+[a-z0-9\-]*)\b/gi;
  while ((m = re2.exec(html)) !== null) {
    out.add(`abab-${m[1]}`);
  }
  return Array.from(out);
}

/**
 * 百度千帆 — 文心一言 ERNIE.
 * 例: ernie-5.0, ernie-4.5-8k, ernie-3.5-8k, ernie-speed-8k
 * 注意 ERNIE 有 2 类命名:带数字版本 (5.0/4.5/3.5) + 带非数字 suffix
 * (speed/lite/pro)。regex 要支持 2 种。
 */
export function extractErnie(html) {
  const out = new Set();
  const re = /ernie[\-_]([a-z0-9\.]+(?:[\-_][a-z0-9\.]+)*)/gi;
  let m;
  while ((m = re.exec(html)) !== null) {
    const v = m[1].toLowerCase();
    // Skip pure "bot" / "turbo" / "functions" — LLM family names not specific
    // model IDs in default_model field.
    if (['bot', 'turbo', 'functions', 'novel', 'character', 'tiny'].includes(v)) continue;
    out.add(`ernie-${v}`);
  }
  return Array.from(out);
}

/**
 * Per-vendor extractor registry. v0.115 加 5 个新 vendor-specific 函数
 * (extractQwen / extractDoubao / extractKimi / extractGlm / extractMiniMax / extractErnie),
 * 主脚本用 provider.id 选对应函数。
 */
export const VENDOR_EXTRACTORS = {
  qwen: extractQwen,
  doubao: extractDoubao,
  kimi: extractKimi,
  glm: extractGlm,
  MiniMax: extractMiniMax,
  ernie: extractErnie,
};

export { genericExtract };
