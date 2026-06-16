import { Zap } from 'lucide-react';
import { Placeholder } from '@/components/ui/Placeholder';

export function Signals() {
  return (
    <Placeholder
      title="Signals"
      icon={<Zap className="w-8 h-8" />}
      hint="Model output across all markets."
      filename="src/routes/Signals.tsx"
    />
  );
}