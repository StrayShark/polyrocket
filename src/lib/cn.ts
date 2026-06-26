import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** 支持 Tailwind 的 className 合并器 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}