import { AlertCircle } from 'lucide-react';

export interface ErrorStateProps {
  title?: string;
  message: string;
  onRetry?: () => void;
}

export function ErrorState({
  title = 'Something went wrong',
  message,
  onRetry,
}: ErrorStateProps) {
  return (
    <div className="flex flex-col items-center justify-center text-center py-8 px-4">
      <div className="w-10 h-10 rounded-full bg-bear/10 text-bear grid place-items-center mb-3">
        <AlertCircle className="w-5 h-5" />
      </div>
      <h3 className="text-[13px] font-medium text-fg">{title}</h3>
      <p className="text-[12px] text-muted mt-1 max-w-md font-mono break-words">{message}</p>
      {onRetry && (
        <button
          onClick={onRetry}
          className="mt-3 text-[12px] text-accent hover:underline"
        >
          Try again
        </button>
      )}
    </div>
  );
}
