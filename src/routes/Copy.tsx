import { Copy as CopyIcon } from 'lucide-react';
import { Placeholder } from '@/components/ui/Placeholder';

export function Copy() {
  return (
    <Placeholder
      title="Copy Trading"
      icon={<CopyIcon className="w-8 h-8" />}
      hint="Track + mirror selected Polymarket addresses."
      filename="src/routes/Copy.tsx"
    />
  );
}