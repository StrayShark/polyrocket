// v0.52 — Trade route. Standalone page hosting
// the PlaceBetForm. Users navigate here to
// manually place a bet. The form is also opened
// from Signal cards (in v0.52b).

import { useSearchParams } from 'react-router-dom';
import { Card } from '@/components/base/Card';
import { useT } from '@/lib/i18n';
import { PlaceBetForm } from '@/components/feedback/PlaceBetForm';
import type { BetSide } from '@/ipc';

export function Trade() {
  const { t } = useT();
  const [params] = useSearchParams();
  // The Signals page links here with query params
  // to pre-fill the form. v0.52b wires this up.
  const marketId = params.get('market') ?? undefined;
  const side = (params.get('side') as BetSide | null) ?? undefined;
  const price = params.get('price');
  const initialPrice = price ? parseFloat(price) : undefined;

  return (
    <div className="space-y-4">
      <Card title={t('trade.title')} description={t('trade.desc')}>
        <p className="text-[12px] text-muted">
          {t('trade.body')}
        </p>
      </Card>
      <PlaceBetForm
        initialMarketId={marketId}
        initialSide={side}
        initialPrice={initialPrice}
      />
    </div>
  );
}
