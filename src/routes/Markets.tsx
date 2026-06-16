import { LineChart } from 'lucide-react';
import { Placeholder } from '@/components/ui/Placeholder';

export function Markets() {
  return (
    <Placeholder
      title="Markets"
      icon={<LineChart className="w-8 h-8" />}
      hint="All Polymarket markets filtered by category. Next: wire to list_markets IPC + TanStack Table."
      filename="src/routes/Markets.tsx"
    />
  );
}