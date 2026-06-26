/**
 * Vitest setup 文件 —— 在每个测试文件之前运行。
 *
 * - 导入 `@testing-library/jest-dom` 以在 .test.tsx 文件中
 *   使用 `toBeInTheDocument()` 等匹配器
 * - （为未来的全局 mock / window.matchMedia shim 预留）
 */
import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

// happy-dom 不会在测试之间自动清理 DOM，会导致
// modal / portal 跨测试泄漏。
afterEach(() => cleanup());
