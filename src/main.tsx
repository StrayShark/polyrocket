import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { RouterProvider, createBrowserRouter, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { applyRetryPolicy } from '@/lib/retry-policy';
import { Dashboard } from '@/routes/Dashboard';
import { Markets } from '@/routes/Markets';
import { MarketDetail } from '@/routes/MarketDetail';
import { Signals } from '@/routes/Signals';
import { Copy } from '@/routes/Copy';
import { PnL } from '@/routes/PnL';
import { Trade } from '@/routes/Trade';
import { ModelLab } from '@/routes/ModelLab';
import { History } from '@/routes/History';
import { Wallets } from '@/routes/Wallets';
import { Settings } from '@/routes/Settings';
import { Analysis } from '@/routes/Analysis';
import { LlmPerf } from '@/routes/LlmPerf';
import { LlmMgmt } from '@/routes/LlmMgmt';
import { Brief } from '@/routes/Brief';
import { Onboarding } from '@/routes/Onboarding';
import { Welcome } from '@/routes/Welcome';
import { Audit } from '@/routes/Audit';
import { Notifications } from '@/routes/Notifications';
import { Help } from '@/routes/Help';
import { AppShell } from '@/components/layout/AppShell';
import { useThemeStore } from '@/stores/theme-store';
import { ToastHost } from '@/components/feedback/Toast';
import { ErrorBoundary } from '@/components/feedback/ErrorBoundary';
import './styles/globals.css';

// Initialize theme on app boot (before paint)
const initialTheme = useThemeStore.getState().theme;
document.documentElement.setAttribute('data-theme', initialTheme);

const queryClient = new QueryClient();
// v0.9a — smarter retry policy: backoff + jitter + respects error kind.
// (Replaces the old hard-coded `retry: 1`.)
applyRetryPolicy(queryClient);

const router = createBrowserRouter([
  {
    path: '/',
    element: <AppShell />,
    children: [
      { index: true, element: <Navigate to="/dashboard" replace /> },
      { path: 'dashboard', element: <Dashboard /> },
      { path: 'markets', element: <Markets /> },
      { path: 'markets/:id', element: <MarketDetail /> },
      { path: 'signals', element: <Signals /> },
      { path: 'copy', element: <Copy /> },
      { path: 'pnl', element: <PnL /> },
      { path: 'trade', element: <Trade /> },
      { path: 'lab', element: <ModelLab /> },
      { path: 'history', element: <History /> },
      { path: 'wallets', element: <Wallets /> },
      { path: 'settings', element: <Settings /> },
      { path: 'analysis', element: <Analysis /> },
      { path: 'llm-perf', element: <LlmPerf /> },
      { path: 'llm-mgmt', element: <LlmMgmt /> },
      { path: 'brief', element: <Brief /> },
      { path: 'onboarding', element: <Onboarding /> },
      { path: 'welcome', element: <Welcome /> },
      { path: 'audit', element: <Audit /> },
      { path: 'notifications', element: <Notifications /> },
      { path: 'help', element: <Help /> },
    ],
  },
]);

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
        <ToastHost />
      </QueryClientProvider>
    </ErrorBoundary>
  </StrictMode>,
);