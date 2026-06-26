import { useEffect, useState, useCallback } from 'react';
import { Card } from '@/components/base/Card';
import { Input } from '@/components/base/Input';
import { Toggle } from '@/components/base/Toggle';
import { useT } from '@/lib/i18n';
import {
  placeSignedOrder,
  validateOrderArgs,
} from '@/ipc';
import type {
  PlaceSignedArgs,
  OrderType,
  BetSide,
} from '@/types/bet';
import { toast } from '@/stores/toast-store';
import { cn } from '@/lib/cn';

export interface PlaceBetFormProps {
  /** 为特定 market 预填表单
   * (例如从 Signal 卡片进入时)。 */
  initialMarketId?: string;
  initialSide?: BetSide;
  initialPrice?: number;
  /** 提交成功后的回调。父组件
   * (例如 Signals 页面)可借此跳转到
   * Bets 页面或刷新列表。 */
  onSuccess?: (betId: string) => void;
}

/**
 * `PlaceBetForm` —— 完整的下单 form（Market / Limit / StopLoss 三种 order type）。
 *
 * **Props**：
 *   - `initialMarketId` — 预填 market（URL `?market_id=...` 进来时 L1 解析）
 *   - `initialSide` — 预填 YES/NO（默认 YES）
 *   - `initialPrice` — 预填 mid price
 *   - `onSuccess(betId)` — submit 成功后回调（路由跳转 / 关闭 modal）
 *
 * **Defaults**：
 *   - `side: 'YES'`（toggle 改 NO）
 *   - `order_type: 'market'`
 *   - `price: 0.5`（mid）
 *   - `size: '10' USDC`
 *   - `post_only: false`
 *
 * **Conditional fields**：
 *   - `limit_price` — order_type = 'limit' OR 'stop_loss'（StopLoss 触发后用 limit 价）
 *   - `stop_price` — order_type = 'stop_loss'
 *   - `post_only` — only when 'limit'（做市单）
 *
 * **Live validation**：调 `validateOrderArgs` IPC（同一函数 Rust 端也会调）——
 * 用户立刻看到「market_id required」/「size > 10000」等错，不用 round-trip submit。
 *
 * **Submit**：调 `placeSignedOrder` IPC → Rust 端走 `validatePlaceArgs` →
 * 调 `placeBet` 写 DB + 调 CLOB。成功 toast + 调 `onSuccess(betId)`。
 */
export function PlaceBetForm(props: PlaceBetFormProps) {
  const { t } = useT();
  // 表单状态
  const [marketId, setMarketId] = useState(props.initialMarketId ?? '');
  const [side, setSide] = useState<BetSide>(props.initialSide ?? 'YES');
  const [size, setSize] = useState('10');
  const [price, setPrice] = useState(String(props.initialPrice ?? 0.5));
  const [orderType, setOrderType] = useState<OrderType>('market');
  const [limitPrice, setLimitPrice] = useState('0.5');
  const [stopPrice, setStopPrice] = useState('0.7');
  const [postOnly, setPostOnly] = useState(false);
  const [keyAlias, setKeyAlias] = useState('primary');
  // UI 状态
  const [validationErr, setValidationErr] = useState<string | null>(null);
  const [validationOk, setValidationOk] = useState<boolean | null>(null);
  const [submitting, setSubmitting] = useState(false);

  // v0.52a —— 实时校验。每当表单变化(由依赖列表 debounce),
  // 调用 validateOrderArgs 并展示结果。
  // Rust 端在 submit 时也会做同样校验,
  // 所以这里只是提前告警的 UX。
  useEffect(() => {
    if (!marketId.trim()) {
      setValidationOk(null);
      setValidationErr(null);
      return;
    }
    const args: PlaceSignedArgs = {
      market_id: marketId,
      wallet_id: 'primary',
      side,
      price: parseFloat(price) || 0,
      size,
      key_alias: keyAlias,
      order_type: orderType,
      limit_price: orderType !== 'market' ? parseFloat(limitPrice) || undefined : undefined,
      stop_price: orderType === 'stop_loss' ? parseFloat(stopPrice) || undefined : undefined,
      post_only: orderType === 'limit' && postOnly,
    };
    let cancelled = false;
    validateOrderArgs(args)
      .then(() => {
        if (cancelled) return;
        setValidationOk(true);
        setValidationErr(null);
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setValidationOk(false);
        setValidationErr(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [marketId, side, size, price, orderType, limitPrice, stopPrice, postOnly, keyAlias]);

  const onSubmit = useCallback(async () => {
    if (validationOk === false) {
      toast.error(validationErr ?? t('place_bet.invalid'));
      return;
    }
    setSubmitting(true);
    try {
      const args: PlaceSignedArgs = {
        market_id: marketId,
        wallet_id: 'primary',
        side,
        price: parseFloat(price),
        size,
        key_alias: keyAlias,
        order_type: orderType,
        limit_price: orderType !== 'market' ? parseFloat(limitPrice) || undefined : undefined,
        stop_price: orderType === 'stop_loss' ? parseFloat(stopPrice) || undefined : undefined,
        post_only: orderType === 'limit' && postOnly,
      };
      const bet = await placeSignedOrder(args);
      toast.success(t('place_bet.placed', { id: bet.id.slice(0, 8) }));
      props.onSuccess?.(bet.id);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setSubmitting(false);
    }
  }, [validationOk, validationErr, marketId, side, price, size, keyAlias, orderType, limitPrice, stopPrice, postOnly, toast, props, t]);

  return (
    <Card
      title={t('place_bet.title')}
      description={t('place_bet.desc')}
    >
      <div className="space-y-3" data-testid="place-bet-form">
        {/* 市场 ID + 方向 行 */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <Field label={t('place_bet.market_id')}>
            <Input
              data-testid="place-bet-market-id"
              value={marketId}
              onChange={(e) => setMarketId(e.target.value)}
              placeholder="0x123abc..."
            />
          </Field>
          <Field label={t('place_bet.side')}>
            <div className="flex gap-2 mt-1">
              <SideButton
                active={side === 'YES'}
                onClick={() => setSide('YES')}
                testid="place-bet-side-yes"
              >
                YES
              </SideButton>
              <SideButton
                active={side === 'NO'}
                onClick={() => setSide('NO')}
                testid="place-bet-side-no"
              >
                NO
              </SideButton>
            </div>
          </Field>
        </div>

        {/* 订单类型选择 */}
        <Field label={t('place_bet.order_type')}>
          <div className="flex gap-2 mt-1">
            <OrderTypeButton
              active={orderType === 'market'}
              onClick={() => setOrderType('market')}
              testid="place-bet-order-type-market"
            >
              {t('order_type.market')}
            </OrderTypeButton>
            <OrderTypeButton
              active={orderType === 'limit'}
              onClick={() => setOrderType('limit')}
              testid="place-bet-order-type-limit"
            >
              {t('order_type.limit')}
            </OrderTypeButton>
            <OrderTypeButton
              active={orderType === 'stop_loss'}
              onClick={() => setOrderType('stop_loss')}
              testid="place-bet-order-type-stop-loss"
            >
              {t('order_type.stop_loss')}
            </OrderTypeButton>
          </div>
        </Field>

        {/* 数量 + 价格 行 */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <Field label={t('place_bet.size_usdc')}>
            <Input
              data-testid="place-bet-size"
              value={size}
              onChange={(e) => setSize(e.target.value)}
              inputMode="decimal"
            />
          </Field>
          <Field label={t('place_bet.price')}>
            <Input
              data-testid="place-bet-price"
              value={price}
              onChange={(e) => setPrice(e.target.value)}
              inputMode="decimal"
            />
          </Field>
        </div>

        {/* 条件字段：limit_price（限价 + 止损）*/}
        {orderType !== 'market' && (
          <Field label={t('place_bet.limit_price')}>
            <Input
              data-testid="place-bet-limit-price"
              value={limitPrice}
              onChange={(e) => setLimitPrice(e.target.value)}
              inputMode="decimal"
            />
          </Field>
        )}

        {/* 条件字段：stop_price（仅止损）*/}
        {orderType === 'stop_loss' && (
          <Field label={t('place_bet.stop_price')}>
            <Input
              data-testid="place-bet-stop-price"
              value={stopPrice}
              onChange={(e) => setStopPrice(e.target.value)}
              inputMode="decimal"
            />
          </Field>
        )}

        {/* 条件字段：post_only（仅限价）*/}
        {orderType === 'limit' && (
          <div className="flex items-center gap-2">
            <Toggle
              data-testid="place-bet-post-only"
              checked={postOnly}
              onChange={setPostOnly}
              label={t('place_bet.post_only')}
            />
          </div>
        )}

        {/* 密钥别名 */}
        <Field label={t('place_bet.key_alias')}>
          <Input
            data-testid="place-bet-key-alias"
            value={keyAlias}
            onChange={(e) => setKeyAlias(e.target.value)}
            placeholder="primary"
          />
        </Field>

        {/* 校验反馈 */}
        {validationOk === false && validationErr && (
          <div
            data-testid="place-bet-validation-error"
            className="text-[10px] text-bear bg-bear/10 rounded px-2 py-1"
          >
            {validationErr}
          </div>
        )}
        {validationOk === true && (
          <div
            data-testid="place-bet-validation-ok"
            className="text-[10px] text-bull"
          >
            ✓ {t('place_bet.valid')}
          </div>
        )}

        {/* 提交 */}
        <button
          type="button"
          onClick={onSubmit}
          disabled={submitting || validationOk === false || !marketId.trim()}
          className={cn(
            'w-full h-9 rounded-md text-body-sm font-medium transition-colors duration-base ease-out-cubic',
            'bg-accent text-bg hover:bg-accent/90',
            'disabled:opacity-50 disabled:cursor-not-allowed',
          )}
          data-testid="place-bet-submit"
        >
          {submitting ? t('place_bet.submitting') : t('place_bet.submit')}
        </button>
      </div>
    </Card>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="text-[10px] text-muted block mb-1">{label}</span>
      {children}
    </label>
  );
}

function SideButton({
  active,
  onClick,
  children,
  testid,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
  testid: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={testid}
      className={cn(
        'flex-1 h-8 rounded-md text-[12px] font-medium transition-colors duration-base ease-out-cubic',
        active
          ? 'bg-accent text-bg'
          : 'bg-surface-2 text-muted hover:text-fg',
      )}
    >
      {children}
    </button>
  );
}

function OrderTypeButton({
  active,
  onClick,
  children,
  testid,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
  testid: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      data-testid={testid}
      className={cn(
        'flex-1 h-8 rounded-md text-[12px] font-medium transition-colors duration-base ease-out-cubic',
        active
          ? 'bg-accent text-bg'
          : 'bg-surface-2 text-muted hover:text-fg',
      )}
    >
      {children}
    </button>
  );
}
