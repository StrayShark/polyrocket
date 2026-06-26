// v0.57d —— 钱包 JSON 文件解析助手。
//
// 钱包通常由 MetaMask / Rabby / frame /
// WalletConnect 合作 dapp 以 JSON 形式
// 导出。各工具的格式
// 略有差异，因此我们接受几种形式并
// 提取第一个合法的 0x 地址：
//
//   1. 简单形式: { "address": "0x..." } —— frame
//   2. Rabby/MetaMask v3+: { "addr": "0x..." }
//   3. 嵌套形式: { "accounts": ["0x..."] } —— WC
//   4. 在任意字段中以 0x 开头的
//      40 字符 hex 字符串
//
// 返回：地址；如果没找到任何
// 以 0x 开头、40 字符的 hex 值则返回 null。

const ADDR_RE = /0x[0-9a-fA-F]{40}/;

export function extractAddressFromJson(content: string): string | null {
  // 1. 先尝试 JSON 解析。如果失败，
  // 回退到对原始内容做正则匹配。
  let obj: unknown = null;
  try {
    obj = JSON.parse(content);
  } catch {
    // 不是 JSON。在原始文本上做正则匹配。
    const m = content.match(ADDR_RE);
    return m ? m[0] : null;
  }
  if (typeof obj === 'string') {
    const m = obj.match(ADDR_RE);
    return m ? m[0] : null;
  }
  if (obj && typeof obj === 'object') {
    // 遍历对象查找地址。
    return walkForAddress(obj as Record<string, unknown>);
  }
  return null;
}

function walkForAddress(node: unknown, depth = 0): string | null {
  if (depth > 6) return null; // 循环 / 深层树保护
  if (typeof node === 'string') {
    const m = node.match(ADDR_RE);
    return m ? m[0] : null;
  }
  if (Array.isArray(node)) {
    for (const v of node) {
      const found = walkForAddress(v, depth + 1);
      if (found) return found;
    }
    return null;
  }
  if (node && typeof node === 'object') {
    // 优先检查标准字段名。
    const obj = node as Record<string, unknown>;
    for (const key of ['address', 'addr', 'account', 'accounts', 'wallet']) {
      if (key in obj) {
        const found = walkForAddress(obj[key], depth + 1);
        if (found) return found;
      }
    }
    // 再遍历所有其他字段。
    for (const v of Object.values(obj)) {
      const found = walkForAddress(v, depth + 1);
      if (found) return found;
    }
  }
  return null;
}
