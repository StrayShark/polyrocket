// v0.53b — PolymarketStep (Step 5 of 6).
//
// Two sub-cards:
//   1. CLOB API credentials (api_key / secret /
//      passphrase). 3 fields, all required. The
//      keyring aliases are surfaced inline.
//   2. Trading wallet (optional). Address +
//      private key.
//
// Each sub-card has its own save + skip. Skipping
// the wallet is normal — many users only use
// mode-A jump bets and don't need a key in keyring.

import { useState, useCallback } from 'react';
import { Card } from '@/components/base/Card';
import { Input } from '@/components/base/Input';
import { Button } from '@/components/base/Button';
import { useT } from '@/lib/i18n';
import { useWelcomeStore } from '@/stores/welcome-store';
import {
  llmPmSetCredentials,
  polyrocketWalletSetPk,
} from '@/ipc';
import { toast } from '@/stores/toast-store';
import { Key, Wallet, CheckCircle2, AlertTriangle, Loader2 } from 'lucide-react';
import { cn } from '@/lib/cn';

export function PolymarketStep({
  welcome,
}: {
  welcome: ReturnType<typeof useWelcomeStore.getState>;
}) {
  const { t } = useT();
  return (
    <div className="space-y-4 py-2">
      <div>
        <h2 className="text-[18px] font-semibold text-fg">
          {t('welcome.pm_title')}
        </h2>
        <p className="text-[12px] text-muted mt-1">
          {t('welcome.pm_desc')}
        </p>
      </div>
      <ClobCard welcome={welcome} />
      <WalletCard welcome={welcome} />
    </div>
  );
}

function ClobCard({
  welcome,
}: {
  welcome: ReturnType<typeof useWelcomeStore.getState>;
}) {
  const { t } = useT();
  const [apiKey, setApiKey] = useState('');
  const [apiSecret, setApiSecret] = useState('');
  const [apiPassphrase, setApiPassphrase] = useState('');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{
    ok: boolean;
    message: string;
  } | null>(null);

  const onSave = useCallback(async () => {
    if (!apiKey.trim() || !apiSecret.trim() || !apiPassphrase.trim()) {
      toast.error(t('welcome.pm_required'));
      return;
    }
    setBusy(true);
    setResult(null);
    try {
      await llmPmSetCredentials(
        apiKey.trim(),
        apiSecret.trim(),
        apiPassphrase.trim(),
      );
      welcome.setConfigured('polymarketApi', true);
      setResult({ ok: true, message: t('welcome.pm_saved') });
      toast.success(t('welcome.pm_saved'));
      setApiKey('');
      setApiSecret('');
      setApiPassphrase('');
    } catch (e) {
      setResult({ ok: false, message: String(e) });
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }, [apiKey, apiSecret, apiPassphrase, welcome, t]);

  return (
    <Card
      title={t('welcome.pm_clob_title')}
      description={t('welcome.pm_clob_desc')}
    >
      <div className="space-y-2.5">
        <Field label={t('welcome.pm_api_key')} testid="welcome-pm-api-key">
          <Input
            data-testid="welcome-pm-api-key-input"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            type="password"
            placeholder="..."
          />
          <div className="text-[10px] text-muted mt-0.5 font-mono">
            polyrocket/pm/api
          </div>
        </Field>
        <Field label={t('welcome.pm_api_secret')} testid="welcome-pm-api-secret">
          <Input
            data-testid="welcome-pm-api-secret-input"
            value={apiSecret}
            onChange={(e) => setApiSecret(e.target.value)}
            type="password"
            placeholder="..."
          />
          <div className="text-[10px] text-muted mt-0.5 font-mono">
            polyrocket/pm/secret
          </div>
        </Field>
        <Field
          label={t('welcome.pm_api_passphrase')}
          testid="welcome-pm-api-passphrase"
        >
          <Input
            data-testid="welcome-pm-api-passphrase-input"
            value={apiPassphrase}
            onChange={(e) => setApiPassphrase(e.target.value)}
            type="password"
            placeholder="..."
          />
          <div className="text-[10px] text-muted mt-0.5 font-mono">
            polyrocket/pm/passphrase
          </div>
        </Field>
        <div className="flex items-center justify-end gap-2 pt-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setApiKey('');
              setApiSecret('');
              setApiPassphrase('');
              setResult(null);
            }}
            data-testid="welcome-pm-skip"
          >
            {t('welcome.pm_skip')}
          </Button>
          <Button
            variant="primary"
            size="sm"
            iconLeft={
              busy ? (
                <Loader2 className="w-3 h-3 animate-spin" />
              ) : (
                <Key className="w-3 h-3" />
              )
            }
            onClick={onSave}
            disabled={busy}
            data-testid="welcome-pm-save"
          >
            {t('welcome.pm_save')}
          </Button>
        </div>
        {result && (
          <div
            data-testid="welcome-pm-result"
            className={cn(
              'rounded-md p-2.5 text-[11px] flex items-center gap-2',
              result.ok
                ? 'bg-bull/10 text-bull'
                : 'bg-bear/10 text-bear',
            )}
          >
            {result.ok ? (
              <CheckCircle2 className="w-3.5 h-3.5" />
            ) : (
              <AlertTriangle className="w-3.5 h-3.5" />
            )}
            <span className="font-mono">{result.message}</span>
          </div>
        )}
      </div>
    </Card>
  );
}

function WalletCard({
  welcome,
}: {
  welcome: ReturnType<typeof useWelcomeStore.getState>;
}) {
  const { t } = useT();
  const [address, setAddress] = useState('');
  const [pk, setPk] = useState('');
  const [alias, setAlias] = useState('primary');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{
    ok: boolean;
    message: string;
  } | null>(null);

  const onSave = useCallback(async () => {
    if (!address.trim() || !pk.trim()) {
      toast.error(t('welcome.wallet_required'));
      return;
    }
    setBusy(true);
    setResult(null);
    try {
      await polyrocketWalletSetPk(alias.trim() || 'primary', pk.trim());
      // The polyrocket_wallet_set_pk IPC currently
      // takes (alias, pk). The address comes from
      // the keyring alias derivation. For v0.53b
      // we just trust the alias + pk pair; the
      // wallet page shows the address derived from
      // the keyring entry.
      welcome.setConfigured('walletPk', true);
      setResult({ ok: true, message: t('welcome.wallet_saved') });
      toast.success(t('welcome.wallet_saved'));
      setPk('');
    } catch (e) {
      setResult({ ok: false, message: String(e) });
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  }, [address, pk, alias, welcome, t]);

  return (
    <Card
      title={t('welcome.wallet_title')}
      description={t('welcome.wallet_desc')}
    >
      <div className="space-y-2.5">
        <Field label={t('welcome.wallet_address')} testid="welcome-wallet-address">
          <Input
            data-testid="welcome-wallet-address-input"
            value={address}
            onChange={(e) => setAddress(e.target.value)}
            placeholder="0x..."
          />
        </Field>
        <Field label={t('welcome.wallet_alias')} testid="welcome-wallet-alias">
          <Input
            data-testid="welcome-wallet-alias-input"
            value={alias}
            onChange={(e) => setAlias(e.target.value)}
            placeholder="primary"
          />
          <div className="text-[10px] text-muted mt-0.5 font-mono">
            polyrocket/wallet/{alias || 'primary'}
          </div>
        </Field>
        <Field label={t('welcome.wallet_pk')} testid="welcome-wallet-pk">
          <Input
            data-testid="welcome-wallet-pk-input"
            value={pk}
            onChange={(e) => setPk(e.target.value)}
            type="password"
            placeholder={t('welcome.wallet_pk_placeholder')}
          />
          <div className="text-[10px] text-bear mt-0.5">
            {t('welcome.wallet_warn')}
          </div>
        </Field>
        <div className="flex items-center justify-end gap-2 pt-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setPk('');
              setResult(null);
            }}
            data-testid="welcome-wallet-skip"
          >
            {t('welcome.wallet_skip')}
          </Button>
          <Button
            variant="primary"
            size="sm"
            iconLeft={
              busy ? (
                <Loader2 className="w-3 h-3 animate-spin" />
              ) : (
                <Wallet className="w-3 h-3" />
              )
            }
            onClick={onSave}
            disabled={busy}
            data-testid="welcome-wallet-save"
          >
            {t('welcome.wallet_save')}
          </Button>
        </div>
        {result && (
          <div
            data-testid="welcome-wallet-result"
            className={cn(
              'rounded-md p-2.5 text-[11px] flex items-center gap-2',
              result.ok
                ? 'bg-bull/10 text-bull'
                : 'bg-bear/10 text-bear',
            )}
          >
            {result.ok ? (
              <CheckCircle2 className="w-3.5 h-3.5" />
            ) : (
              <AlertTriangle className="w-3.5 h-3.5" />
            )}
            <span className="font-mono">{result.message}</span>
          </div>
        )}
      </div>
    </Card>
  );
}

function Field({
  label,
  children,
  testid,
}: {
  label: string;
  children: React.ReactNode;
  testid: string;
}) {
  return (
    <div data-testid={testid}>
      <span className="text-[10px] text-muted block mb-1 uppercase tracking-wider">
        {label}
      </span>
      {children}
    </div>
  );
}
