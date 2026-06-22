// polyrocket — per-vendor model ID extractors (v0.115) tests.
//
// 验证 v0.115 重构后 6 个 vendor extractor 各自能从 vendor HTML 抽
// 出正确 model IDs,主脚本调用结果跟 v0.112 inline regex 一致。
//
// Run: `node --test scripts/vendor-parsers.test.mjs`

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  extractQwen,
  extractDoubao,
  extractKimi,
  extractGlm,
  extractMiniMax,
  extractErnie,
  VENDOR_EXTRACTORS,
  genericExtract,
} from './vendor-parsers.mjs';

describe('extractQwen', () => {
  it('extracts qwen 3.x max/plus/flash variants', () => {
    const html = `
      <ul>
        <li>qwen3.7-max (latest)</li>
        <li>qwen3.7-plus</li>
        <li>qwen3.6-flash</li>
      </ul>
    `;
    const r = extractQwen(html);
    assert.ok(r.includes('qwen3.7-max'));
    assert.ok(r.includes('qwen3.7-plus'));
    assert.ok(r.includes('qwen3.6-flash'));
  });

  it('excludes vision (vl) and coder variants', () => {
    const html = `<li>qwen3.7-vl-max</li><li>qwen-coder-7b</li><li>qwen3.7-max</li>`;
    const r = extractQwen(html);
    assert.ok(!r.some((x) => x.includes('vl')));
    assert.ok(!r.some((x) => x.includes('coder')));
    assert.ok(r.includes('qwen3.7-max'));
  });

  it('returns empty array for HTML with no Qwen models', () => {
    assert.deepEqual(extractQwen('<p>no models here</p>'), []);
  });
});

describe('extractDoubao', () => {
  it('extracts doubao-seed and doubao-1-5 variants', () => {
    const html = `<li>doubao-seed-2-0-pro-260215</li><li>doubao-1-5-pro-32k</li>`;
    const r = extractDoubao(html);
    assert.ok(r.includes('doubao-seed-2-0-pro-260215'));
    assert.ok(r.includes('doubao-1-5-pro-32k'));
  });

  it('deduplicates multiple mentions of same model', () => {
    const html = `<li>doubao-1-5-pro</li><li>doubao-1-5-pro</li><li>doubao-1-5-pro</li>`;
    const r = extractDoubao(html);
    assert.equal(r.length, 1);
  });
});

describe('extractKimi', () => {
  it('extracts kimi-k2.x variants', () => {
    const html = `<li>kimi-k2.7-code</li><li>kimi-k2-0711-preview</li>`;
    const r = extractKimi(html);
    assert.ok(r.some((x) => x.includes('k2.7')));
    assert.ok(r.some((x) => x.includes('k2-0711')));
  });
});

describe('extractGlm', () => {
  it('extracts glm-5.2 and glm-4-flash', () => {
    const html = `<li>glm-5.2</li><li>glm-4-plus</li><li>glm-4-flash</li>`;
    const r = extractGlm(html);
    assert.ok(r.includes('glm-5.2'));
    assert.ok(r.includes('glm-4-plus'));
    assert.ok(r.includes('glm-4-flash'));
  });
});

describe('extractMiniMax', () => {
  it('extracts MiniMax-M2.7 and abab-7', () => {
    const html = `<li>MiniMax-M2.7</li><li>MiniMax-Text-01</li><li>abab-7</li>`;
    const r = extractMiniMax(html);
    assert.ok(r.some((x) => x.startsWith('MiniMax-')));
    assert.ok(r.some((x) => x.startsWith('abab-')));
  });
});

describe('extractErnie', () => {
  it('extracts ernie-5.0, ernie-4.5-8k, ernie-speed-8k', () => {
    const html = `<li>ernie-5.0</li><li>ernie-4.5-8k</li><li>ernie-speed-8k</li>`;
    const r = extractErnie(html);
    assert.ok(r.some((x) => x.includes('5.0')));
    assert.ok(r.some((x) => x.includes('4.5')));
    assert.ok(r.some((x) => x.includes('speed')));
  });
});

describe('VENDOR_EXTRACTORS registry', () => {
  it('has all 6 Chinese LLM providers', () => {
    const expected = ['qwen', 'doubao', 'kimi', 'glm', 'MiniMax', 'ernie'];
    for (const id of expected) {
      assert.ok(id in VENDOR_EXTRACTORS, `missing ${id} in VENDOR_EXTRACTORS`);
    }
  });

  it('all extractors are functions', () => {
    for (const [id, fn] of Object.entries(VENDOR_EXTRACTORS)) {
      assert.equal(typeof fn, 'function', `${id} should be a function`);
    }
  });
});

describe('genericExtract fallback', () => {
  it('extracts vendor-name + version pattern', () => {
    const html = `<li>testmodel-3.7-max</li><li>testmodel-2.5</li>`;
    const r = genericExtract(html, 'testmodel');
    assert.ok(r.some((x) => x.includes('3.7')));
    assert.ok(r.some((x) => x.includes('2.5')));
  });

  it('returns empty for non-matching vendor', () => {
    const html = `<li>qwen-3.7-max</li>`;
    const r = genericExtract(html, 'minimax');
    assert.equal(r.length, 0);
  });
});
