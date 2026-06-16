import { BarChart3 } from 'lucide-react';
import { Placeholder } from '@/components/ui/Placeholder';

export function PnL() {
  return (
    <Placeholder
      title="P&L"
      icon={<BarChart3 className="w-8 h-8" />}
      hint="Equity curve, settled bets, ROI by category."
      filename="src/routes/PnL.tsx"
    />
  );
}