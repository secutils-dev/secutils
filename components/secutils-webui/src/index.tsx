import './index.css';

import { lazy, Suspense } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter, Navigate, Route, Routes } from 'react-router';

import { AppContainer } from './app_container';
import { PageLoadingState } from './components';
import { WorkspacePage } from './pages';

const SigninPage = lazy(() => import('./pages/signin'));
const SignupPage = lazy(() => import('./pages/signup'));
const ActivatePage = lazy(() => import('./pages/activate'));

const IndexPage = () => {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<AppContainer />}>
          <Route index element={<Navigate to="/ws" replace />} />
          <Route path="*" element={<Navigate to="/ws" replace />} />
          <Route path="ws" element={<WorkspacePage />} />
          <Route path="ws/:util/:deepLink?" element={<WorkspacePage />} />
          <Route
            path="signin"
            element={
              <Suspense fallback={<PageLoadingState />}>
                <SigninPage />
              </Suspense>
            }
          />
          <Route
            path="signup"
            element={
              <Suspense fallback={<PageLoadingState />}>
                <SignupPage />
              </Suspense>
            }
          />
          <Route
            path="activate"
            element={
              <Suspense fallback={<PageLoadingState />}>
                <ActivatePage />
              </Suspense>
            }
          />
        </Route>
      </Routes>
    </BrowserRouter>
  );
};

createRoot(document.getElementById('root') as Element).render(<IndexPage />);
