// v0.57d — env 文件解析助手。
//
// LLM 管理页允许用户从磁盘选择
// .env / .key / .txt 文件来导入
// secret。选择器返回绝对路径；
// 我们通过 Tauri fs 插件
// （或在测试中使用浏览器 File API）读取文件，
// 并提取 API key。
//
// ## 格式
//
// 接受两种形式：
//
//   1. 纯 key 文件 —— 文件内容本身就是
//      secret。例如单行的 `sk-abc123...`。
//      文件通常 <1024 字节。
//
//   2. .env 形式 —— `KEY=VALUE` 行，可能
//      包含注释。我们解析第一个
//      具有非空值的行，忽略：
//        - 以 `#` 开头的行（注释）
//        - 空行
//        - value 为空的行
//      我们不固定 key 名称；用户
//      可以使用 `OPENAI_API_KEY`、
//      `ANTHROPIC_API_KEY` 等任意名称。
//      选择器界面过滤为 .env / .key / .txt，
//      但解析器是宽松的。
//
// 返回：secret 字符串，如果
// 文件为空 / 未找到 value 行则返回 `null`。

/** 以文本方式读取文件。在 Tauri 构建中，
 * 通过 tauri-plugin-fs API（v0.57d
 * 作为依赖加入）进行读取。在测试 / web 中
 * 使用浏览器 FileReader。 */
export async function readFileText(path: string): Promise<string> {
  // 首先尝试 Tauri fs 插件。动态
  // import 以避免 web 构建报错（该
  // 插件未在 Vite dev
  // server 中注册）。
  try {
    const { readTextFile } = await import(
      /* @vite-ignore */ '@tauri-apps/plugin-fs'
    );
    return await readTextFile(path);
  } catch {
    // Web 后备方案：使用 file:// URL 进行 fetch。
    // 适用于 mock 了 IPC 层的测试，
    // 但不能用于真实的生产 Tauri 构建。
    const res = await fetch(path);
    if (!res.ok) {
      throw new Error(`readFileText: HTTP ${res.status}`);
    }
    return await res.text();
  }
}

/** 解析 .env 形式的文件并返回第一个
 * 非空 KEY=VALUE 对的 value。如果
 * 未找到 value 行则返回 null。 */
export function extractSecretFromEnv(content: string): string | null {
  const lines = content.split(/\r?\n/);
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) continue;
    if (line.startsWith('#')) continue;
    const eq = line.indexOf('=');
    if (eq === -1) {
      // 没有 `=` —— 将整行视为
      // secret（纯 key 文件）。这是
      // `sk-...` 文件的常见形式。
      return line;
    }
    const value = line.slice(eq + 1).trim();
    // 如果存在包裹引号则去除。
    const unquoted = value.replace(/^["'](.*)["']$/, '$1');
    if (unquoted) return unquoted;
  }
  return null;
}
