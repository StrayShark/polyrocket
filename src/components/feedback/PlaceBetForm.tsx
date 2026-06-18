// v0.52a — Place-bet form component.
//
// Renders a form for the user to construct an order
// (Market / Limit / StopLoss) and submit it via
// `placeSignedOrder`. Validates live via
// `validateOrderArgs` so the user sees the same
// errors the Rust side would surface, without a
// round-trip.
//
// Defaults:
//   - side: 'YES' (toggleable)
//   - order_type: 'market'
//   - price: 0.5 (mid)
//   - size: '10' USDC
//   - post_only: false
//
// Conditional fields:
//   - limit_price: shown when order_type = 'limit' OR
//     'stop_loss' (StopLoss uses limit as the fill
//     price once the stop triggers)
//   - stop_price: shown when order_type = 'stop_loss'
//   - post_only: shown only when order_type = 'limit'

import { useEffect, useState, useCallback } from 'react';
import { Card } from '@/components/base/Card';
import { Input } from '@/components/base/Input';
import { Toggle } from '@/components/base/Toggle';
import { useT } from '@/lib/i18n';
import {
  placeSignedOrder,
  validateOrderArgs,
  type PlaceSignedArgs,
  type OrderType,
  type BetSide,
} from '@/ipc';
import { toast } from '@/stores/toast-store';
import { cn } from '@/lib/cn';

export interface PlaceBetFormProps {
  /** Pre-fill the form for a specific market
   * (e.g. when launched from a Signal card). */
  initialMarketId?: string;
  initialSide?: BetSide;
  initialPrice?: number;
  /** Called after a successful submit. The parent
   * (e.g. Signals page) can use it to navigate to
   * the Bets page or refresh a list. */
  onSuccess?: (betId: string) => void;
}

export function PlaceBetForm(props: PlaceBetFormProps) {
  const { t } = useT();
  // Form state
  const [marketId, setMarketId] = useState(props.initialMarketId ?? '');
  const [side, setSide] = useState<BetSide>(props.initialSide ?? 'YES');
  const [size, setSize] = useState('10');
  const [price, setPrice] = useState(String(props.initialPrice ?? 0.5));
  const [orderType, setOrderType] = useState<OrderType>('market');
  const [limitPrice, setLimitPrice] = useState('0.5');
  const [stopPrice, setStopPrice] = useState('0.7');
  const [postOnly, setPostOnly] = useState(false);
  const [keyAlias, setKeyAlias] = useState('primary');
  // UI state
  const [validationErr, setValidationErr] = useState<string | null>(null);
  const [validationOk, setValidationOk] = useState<boolean | null>(null);
  const [submitting, setSubmitting] = useState(false);

  // v0.52a — live validation. Whenever the form
  // changes (debounced via the dep list), call
  // validateOrderArgs and surface the result.
  // The Rust side does the same validation on
  // submit, so this is just an early-warning UX.
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
        {/* Market ID + side row */}
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

        {/* Order type select */}
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

        {/* Size + price row */}
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

        {/* Conditional: limit_price (Limit + StopLoss) */}
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

        {/* Conditional: stop_price (StopLoss only) */}
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

        {/* Conditional: post_only (Limit only) */}
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

        {/* Key alias */}
        <Field label={t('place_bet.key_alias')}>
          <Input
            data-testid="place-bet-key-alias"
            value={keyAlias}
            onChange={(e) => setKeyAlias(e.target.value)}
            placeholder="primary"
          />
        </Field>

        {/* Validation feedback */}
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

        {/* Submit */}
        <button
          type="button"
          onClick={onSubmit}
          disabled={submitting || validationOk === false || !marketId.trim()}
          className={cn(
            'w-full h-9 rounded-md text-[13px] font-medium transition-colors',
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
        'flex-1 h-8 rounded-md text-[12px] font-medium transition-colors',
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
        'flex-1 h-8 rounded-md text-[12px] font-medium transition-colors',
        active
          ? 'bg-accent text-bg'
          : 'bg-surface-2 text-muted hover:text-fg',
      )}
    >
      {children}
    </button>
  );
}
