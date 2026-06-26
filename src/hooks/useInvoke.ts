import { useQuery, type UseQueryOptions } from '@tanstack/react-query';

/**
 * 将 IPC 调用包装为带类型的 React Query hook。
 * 分层规则：这是唯一调用 ipc.ts 的地方。
 */
export function useInvoke<TData, TArgs = void>(
  key: readonly unknown[],
  fn: (args: TArgs) => Promise<TData>,
  args: TArgs,
  options?: Omit<UseQueryOptions<TData>, 'queryKey' | 'queryFn'>,
) {
  return useQuery<TData>({
    queryKey: [...key, args],
    queryFn: () => fn(args),
    ...options,
  });
}
