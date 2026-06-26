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
// v0.57a —— /onboarding 路由已移除
// （在 v0.53b 中被 /welcome 取代）。
// 旧的 Onboarding.tsx 保留为 .unused 以备
// 考古，但不再被任何地方导入。
import { Welcome } from '@/routes/Welcome';
import { Audit } from '@/routes/Audit';
import { Notifications } from '@/routes/Notifications';
import { Bankroll } from '@/routes/Bankroll';  // v0.78 — M11
import { Help } from '@/routes/Help';
import { ArbBoard } from '@/routes/ArbBoard';  // v0.126 — P1-4 arbitrage board
import { AppShell } from '@/components/layout/AppShell';
import { useThemeStore } from '@/stores/theme-store';
import { ToastHost } from '@/components/feedback/Toast';
import { ErrorBoundary } from '@/components/feedback/ErrorBoundary';
import './styles/globals.css';

// 在应用启动时初始化主题（在 paint 之前）
const initialTheme = useThemeStore.getState().theme;
document.documentElement.setAttribute('data-theme', initialTheme);

const queryClient = new QueryClient();
// v0.9a —— 更智能的重试策略：backoff + jitter +
// 尊重错误种类。（取代了原来硬编码的 `retry: 1`。）
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
      { path: 'welcome', element: <Welcome /> },
      { path: 'audit', element: <Audit /> },
      { path: 'notifications', element: <Notifications /> },
      { path: 'bankroll', element: <Bankroll /> },  // v0.78 —— M11
      { path: 'help', element: <Help /> },
      { path: 'arb-board', element: <ArbBoard /> },  // v0.126 —— P1-4
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