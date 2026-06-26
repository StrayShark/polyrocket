// v0.52 — Trade 路由。托管 PlaceBetForm 的独立页。
// 用户导航至此手动下单。v0.52b 起该表单也可从
// Signal 卡片打开。

import { useSearchParams } from 'react-router-dom';
import { Card } from '@/components/base/Card';
import { useT } from '@/lib/i18n';
import { PlaceBetForm } from '@/components/feedback/PlaceBetForm';
import type { BetSide } from '@/types/bet';

/**
 * `/trade` 路由 —— 独立下单页。
 *
 * **使用方式**：
 *   1. 直接访问（手动从 nav 进来）
 *   2. 从 Signal 卡片跳过来（v0.52b+，URL 携带 `?market_id=...&side=YES&edge=...`）
 *
 * **核心 component**：`PlaceBetForm`（@/components/feedback/PlaceBetForm）——
 * 完整的 form + 3 种 order type + post-only 校验 + submit。
 *
 * **URL params**：`market_id` 预填 / `side` 预填 / `edge` hint 展示。
 */
export function Trade() {
  const { t } = useT();
  const [params] = useSearchParams();
  // Signals 页面通过 query 参数链接到此处以预填表单。
  // v0.52b 起实现这一链接。
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
