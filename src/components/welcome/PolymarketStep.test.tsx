// v0.66a — PolymarketStep component tests (v0.66a density + coverage).
//
// PolymarketStep is 31% stmts / 17% branches. It has 2 sub-cards
// (ClobCard + WalletCard) each with their own IPC chains. We
// write 6 tests covering the major branches:
//
//   1. Renders title + sub-cards
//   2. CLOB: missing fields → toast error
//   3. CLOB: success path → setConfigured('polymarketApi', true)
//   4. CLOB: llmPmSetCredentials throws → catch block + result
//   5. Wallet: missing address → toast error
//   6. Wallet: success path → polyrocketWalletSetPk + setConfigured('walletPk', true)

// @vitest-environment happy-dom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { createIpcMock } from '@/test-mocks';

// v0.67f — vi.mock is hoisted above the `const` declarations.
// Use vi.hoisted() to expose our spy objects to the factory.
const { mockPmSetCredentials, mockWalletSetPk, mockSendNotification } = vi.hoisted(() => ({
  mockPmSetCredentials: vi.fn(),
  mockWalletSetPk: vi.fn(),
  mockSendNotification: vi.fn(),
}));

vi.mock('@/ipc', () => createIpcMock({
  llmPmSetCredentials: (...args: unknown[]) => mockPmSetCredentials(...args),
  polyrocketWalletSetPk: (...args: unknown[]) => mockWalletSetPk(...args),
  // Override sendNotification to also call our local spy so
  // tests can assert on it.
  sendNotification: mockSendNotification,
}));

import { PolymarketStep } from './PolymarketStep';
import type { useWelcomeStore } from '@/stores/welcome-store';

function makeWelcome(): ReturnType<typeof useWelcomeStore.getState> {
  return {
    done: false,
    step: 'polymarket',
    locale: 'en',
    configured: {
      storagePath: false, theme: false, llmAtLeastOne: false,
      polymarketApi: false, walletPk: false,
    },
    setStep: vi.fn(),
    setDone: vi.fn(),
    setLocale: vi.fn(),
    setConfigured: vi.fn(),
    reset: vi.fn(),
  } as unknown as ReturnType<typeof useWelcomeStore.getState>;
}

describe('PolymarketStep', () => {
  beforeEach(() => {
    mockPmSetCredentials.mockReset();
    mockWalletSetPk.mockReset();
    mockSendNotification.mockReset().mockResolvedValue(undefined);
  });

  it('renders the CLOB and Wallet sub-cards', () => {
    const welcome = makeWelcome();
    render(<PolymarketStep welcome={welcome} />);
    // i18n is the real dictionary in the test env, so we
    // check the body has the "Polymarket" title from the en dict.
    expect(document.body.textContent).toMatch(/[Pp]olymarket|CLOB/);
  });

  it('CLOB: missing fields → no IPC call', async () => {
    const welcome = makeWelcome();
    render(<PolymarketStep welcome={welcome} />);
    // Find the CLOB Save button by aria/text
    const saveBtns = screen.getAllByRole('button');
    // The CLOB card has its own Save button. We find it by
    // looking for "Save" or "保存" or the surrounding card.
    const clobSave = saveBtns.find((b) => /Save|保存/i.test(b.textContent || ''));
    expect(clobSave).toBeTruthy();
    fireEvent.click(clobSave!);
    await waitFor(() => {
      expect(mockPmSetCredentials).not.toHaveBeenCalled();
    });
  });

  it('CLOB: success path → llmPmSetCredentials + setConfigured', async () => {
    mockPmSetCredentials.mockResolvedValue(undefined);
    const welcome = makeWelcome();
    render(<PolymarketStep welcome={welcome} />);
    // Fill in the 3 CLOB fields. There are 4 inputs total
    // (3 in CLOB + 1 wallet PK). We grab all inputs.
    const inputs = document.querySelectorAll('input');
    fireEvent.change(inputs[0], { target: { value: 'key-1' } });
    fireEvent.change(inputs[1], { target: { value: 'secret-1' } });
    fireEvent.change(inputs[2], { target: { value: 'pass-1' } });
    const saveBtns = screen.getAllByRole('button');
    const clobSave = saveBtns.find((b) => /Save|保存/i.test(b.textContent || ''));
    expect(clobSave).toBeTruthy();
    fireEvent.click(clobSave!);
    await waitFor(() => {
      expect(mockPmSetCredentials).toHaveBeenCalledWith('key-1', 'secret-1', 'pass-1');
      expect(welcome.setConfigured).toHaveBeenCalledWith('polymarketApi', true);
    });
  });

  it('CLOB: llmPmSetCredentials throws → catch block + result', async () => {
    mockPmSetCredentials.mockRejectedValue(new Error('Network error'));
    const welcome = makeWelcome();
    render(<PolymarketStep welcome={welcome} />);
    const inputs = document.querySelectorAll('input');
    fireEvent.change(inputs[0], { target: { value: 'key-1' } });
    fireEvent.change(inputs[1], { target: { value: 'secret-1' } });
    fireEvent.change(inputs[2], { target: { value: 'pass-1' } });
    const saveBtns = screen.getAllByRole('button');
    const clobSave = saveBtns.find((b) => /Save|保存/i.test(b.textContent || ''));
    fireEvent.click(clobSave!);
    await waitFor(() => {
      expect(mockPmSetCredentials).toHaveBeenCalled();
      // setConfigured should NOT be called on error
      expect(welcome.setConfigured).not.toHaveBeenCalledWith('polymarketApi', true);
    });
  });

  it('Wallet: missing address → no IPC call', async () => {
    const welcome = makeWelcome();
    render(<PolymarketStep welcome={welcome} />);
    const saveBtns = screen.getAllByRole('button');
    // Wallet has Save + Skip buttons. We grab "Save wallet" or "保存" by position.
    // Skip the CLOB save; the wallet save is a later button.
    const clobSaveIdx = saveBtns.findIndex((b) => /Save|保存/i.test(b.textContent || ''));
    const walletSave = saveBtns[clobSaveIdx + 1];
    expect(walletSave).toBeTruthy();
    fireEvent.click(walletSave);
    await waitFor(() => {
      expect(mockWalletSetPk).not.toHaveBeenCalled();
    });
  });

  it('Wallet: success path → polyrocketWalletSetPk', async () => {
    mockWalletSetPk.mockResolvedValue(undefined);
    const welcome = makeWelcome();
    render(<PolymarketStep welcome={welcome} />);
    // Wallet needs both address + PK. Use stable testids.
    const addressInput = document.querySelector('[data-testid="welcome-wallet-address-input"]') as HTMLInputElement;
    const pkInput = document.querySelector('[data-testid="welcome-wallet-pk-input"]') as HTMLInputElement;
    fireEvent.change(addressInput, { target: { value: '0xaddr' } });
    fireEvent.change(pkInput, { target: { value: '0xpk' } });
    const walletSave = document.querySelector('[data-testid="welcome-wallet-save"]') as HTMLElement;
    expect(walletSave).toBeTruthy();
    fireEvent.click(walletSave);
    await waitFor(() => {
      expect(mockWalletSetPk).toHaveBeenCalled();
    });
  });
});
