import { useQuery, type UseQueryOptions } from '@tanstack/react-query';

/**
 * Wrap an IPC call into a typed React Query hook.
 * Layer rules: this is the ONLY place that calls ipc.ts.
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
