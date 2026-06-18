// v0.57d — wallet JSON file parsing helper.
//
// Wallets are typically exported as JSON by
// MetaMask / Rabby / frame / WalletConnect
// partner dapps. The format varies slightly
// across tools, so we accept a few shapes and
// extract the first valid 0x address:
//
//   1. Plain: { "address": "0x..." } — frame
//   2. Rabby/MetaMask v3+: { "addr": "0x..." }
//   3. Nested: { "accounts": ["0x..."] } — WC
//   4. Just a 0x-prefixed 40-char hex string
//      in any field
//
// Returns: the address, or null if no
// 0x-prefixed 40-char hex value is found.

const ADDR_RE = /0x[0-9a-fA-F]{40}/;

export function extractAddressFromJson(content: string): string | null {
  // 1. Try JSON parse first. If it fails, fall
  // back to regex on the raw content.
  let obj: unknown = null;
  try {
    obj = JSON.parse(content);
  } catch {
    // Not JSON. Try regex on the raw text.
    const m = content.match(ADDR_RE);
    return m ? m[0] : null;
  }
  if (typeof obj === 'string') {
    const m = obj.match(ADDR_RE);
    return m ? m[0] : null;
  }
  if (obj && typeof obj === 'object') {
    // Walk the object looking for the address.
    return walkForAddress(obj as Record<string, unknown>);
  }
  return null;
}

function walkForAddress(node: unknown, depth = 0): string | null {
  if (depth > 6) return null; // cycle / deep tree guard
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
    // Prefer the standard field names first.
    const obj = node as Record<string, unknown>;
    for (const key of ['address', 'addr', 'account', 'accounts', 'wallet']) {
      if (key in obj) {
        const found = walkForAddress(obj[key], depth + 1);
        if (found) return found;
      }
    }
    // Then walk all other fields.
    for (const v of Object.values(obj)) {
      const found = walkForAddress(v, depth + 1);
      if (found) return found;
    }
  }
  return null;
}
