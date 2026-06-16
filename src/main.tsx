import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { RouterProvider, createBrowserRouter, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Dashboard } from '@/routes/Dashboard';
import { Markets } from '@/routes/Markets';
import { MarketDetail } from '@/routes/MarketDetail';
import { Signals } from '@/routes/Signals';
import { Copy } from '@/routes/Copy';
import { PnL } from '@/routes/PnL';
import { ModelLab } from '@/routes/ModelLab';
import { AppShell } from '@/components/layout/AppShell';
import { useThemeStore } from '@/stores/theme-store';
import { ToastHost } from '@/components/feedback/Toast';
import './styles/globals.css';

// Initialize theme on app boot (before paint)
const initialTheme = useThemeStore.getState().theme;
document.documentElement.setAttribute('data-theme', initialTheme);

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

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
      { path: 'lab', element: <ModelLab /> },
    ],
  },
]);

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <ToastHost />
    </QueryClientProvider>
  </StrictMode>,
);