import { FlaskConical } from 'lucide-react';
import { Placeholder } from '@/components/ui/Placeholder';

export function ModelLab() {
  return (
    <Placeholder
      title="Model Lab"
      icon={<FlaskConical className="w-8 h-8" />}
      hint="Train + evaluate prediction models."
      filename="src/routes/ModelLab.tsx"
    />
  );
}