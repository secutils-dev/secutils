import './index.css';

import { icon as EuiIconSecurityApp } from '@elastic/eui/es/components/icon/assets/app_security';
import { icon as EuiIconApps } from '@elastic/eui/es/components/icon/assets/apps';
import { icon as EuiIconBell } from '@elastic/eui/es/components/icon/assets/bell';
import { icon as EuiIconBoxesVertical } from '@elastic/eui/es/components/icon/assets/boxes_vertical';
import { icon as EuiIconCalendar } from '@elastic/eui/es/components/icon/assets/calendar';
import { icon as EuiIconCheck } from '@elastic/eui/es/components/icon/assets/check';
import { icon as EuiIconChevronLimitLeft } from '@elastic/eui/es/components/icon/assets/chevron_limit_left';
import { icon as EuiIconChevronLimitRight } from '@elastic/eui/es/components/icon/assets/chevron_limit_right';
import { icon as EuiIconChevronSingleDown } from '@elastic/eui/es/components/icon/assets/chevron_single_down';
import { icon as EuiIconChevronSingleLeft } from '@elastic/eui/es/components/icon/assets/chevron_single_left';
import { icon as EuiIconChevronSingleRight } from '@elastic/eui/es/components/icon/assets/chevron_single_right';
import { icon as EuiIconChevronSingleUp } from '@elastic/eui/es/components/icon/assets/chevron_single_up';
import { icon as EuiIconClock } from '@elastic/eui/es/components/icon/assets/clock';
import { icon as EuiIconComment } from '@elastic/eui/es/components/icon/assets/comment';
import { icon as EuiIconControls } from '@elastic/eui/es/components/icon/assets/controls';
import { icon as EuiIconCopy } from '@elastic/eui/es/components/icon/assets/copy';
import { icon as EuiIconCopyClipboard } from '@elastic/eui/es/components/icon/assets/copy_clipboard';
import { icon as EuiIconCross } from '@elastic/eui/es/components/icon/assets/cross';
import { icon as EuiIconDocumentation } from '@elastic/eui/es/components/icon/assets/documentation';
import { icon as EuiIconDot } from '@elastic/eui/es/components/icon/assets/dot';
import { icon as EuiIconDownload } from '@elastic/eui/es/components/icon/assets/download';
import { icon as EuiIconDragVertical } from '@elastic/eui/es/components/icon/assets/drag_vertical';
import { icon as EuiIconEmpty } from '@elastic/eui/es/components/icon/assets/empty';
import { icon as EuiIconExternal } from '@elastic/eui/es/components/icon/assets/external';
import { icon as EuiIconEyeSlash } from '@elastic/eui/es/components/icon/assets/eye_slash';
import { icon as EuiIconFullScreen } from '@elastic/eui/es/components/icon/assets/full_screen';
import { icon as EuiIconFullScreenExit } from '@elastic/eui/es/components/icon/assets/full_screen_exit';
import { icon as EuiIconFunction } from '@elastic/eui/es/components/icon/assets/function';
import { icon as EuiIconGear } from '@elastic/eui/es/components/icon/assets/gear';
import { icon as EuiIconGlobe } from '@elastic/eui/es/components/icon/assets/globe';
import { icon as EuiIconHelp } from '@elastic/eui/es/components/icon/assets/help';
import { icon as EuiIconHome } from '@elastic/eui/es/components/icon/assets/home';
import { icon as EuiIconInputOutput } from '@elastic/eui/es/components/icon/assets/input_output';
import { icon as EuiIconKeyboard } from '@elastic/eui/es/components/icon/assets/keyboard';
import { icon as EuiIconLink } from '@elastic/eui/es/components/icon/assets/link';
import { icon as EuiIconLogOut } from '@elastic/eui/es/components/icon/assets/log_out';
import { icon as EuiIconMagnify } from '@elastic/eui/es/components/icon/assets/magnify';
import { icon as EuiIconMail } from '@elastic/eui/es/components/icon/assets/mail';
import { icon as EuiIconMaximize } from '@elastic/eui/es/components/icon/assets/maximize';
import { icon as EuiIconMinus } from '@elastic/eui/es/components/icon/assets/minus';
import { icon as EuiIconMinusCircle } from '@elastic/eui/es/components/icon/assets/minus_circle';
import { icon as EuiIconPayment } from '@elastic/eui/es/components/icon/assets/payment';
import { icon as EuiIconPencil } from '@elastic/eui/es/components/icon/assets/pencil';
import { icon as EuiIconPlusCircle } from '@elastic/eui/es/components/icon/assets/plus_circle';
import { icon as EuiIconPopper } from '@elastic/eui/es/components/icon/assets/popper';
import { icon as EuiIconPresentation } from '@elastic/eui/es/components/icon/assets/presentation';
import { icon as EuiIconQuestion } from '@elastic/eui/es/components/icon/assets/question';
import { icon as EuiIconRadar } from '@elastic/eui/es/components/icon/assets/radar';
import { icon as EuiIconRefresh } from '@elastic/eui/es/components/icon/assets/refresh';
import { icon as EuiIconRefreshTime } from '@elastic/eui/es/components/icon/assets/refresh_time';
import { icon as EuiIconReturn } from '@elastic/eui/es/components/icon/assets/return';
import { icon as EuiIconScissors } from '@elastic/eui/es/components/icon/assets/scissors';
import { icon as EuiIconSecuritySignalDetected } from '@elastic/eui/es/components/icon/assets/security_signal_detected';
import { icon as EuiIconShare } from '@elastic/eui/es/components/icon/assets/share';
import { icon as EuiIconSortDown } from '@elastic/eui/es/components/icon/assets/sort_down';
import { icon as EuiIconSortLeft } from '@elastic/eui/es/components/icon/assets/sort_left';
import { icon as EuiIconSortRight } from '@elastic/eui/es/components/icon/assets/sort_right';
import { icon as EuiIconSortUp } from '@elastic/eui/es/components/icon/assets/sort_up';
import { icon as EuiIconSortable } from '@elastic/eui/es/components/icon/assets/sortable';
import { icon as EuiIconStar } from '@elastic/eui/es/components/icon/assets/star';
import { icon as EuiIconStarFill } from '@elastic/eui/es/components/icon/assets/star_fill';
import { icon as EuiIconTable } from '@elastic/eui/es/components/icon/assets/table';
import { icon as EuiIconTableDensityHigh } from '@elastic/eui/es/components/icon/assets/table_density_high';
import { icon as EuiIconTableDensityLow } from '@elastic/eui/es/components/icon/assets/table_density_low';
import { icon as EuiIconTokenKey } from '@elastic/eui/es/components/icon/assets/token_key';
import { icon as EuiIconTokenNumber } from '@elastic/eui/es/components/icon/assets/token_number';
import { icon as EuiIconTokenString } from '@elastic/eui/es/components/icon/assets/token_string';
import { icon as EuiIconTrash } from '@elastic/eui/es/components/icon/assets/trash';
import { icon as EuiIconUser } from '@elastic/eui/es/components/icon/assets/user';
import { icon as EuiIconVectorTriangle } from '@elastic/eui/es/components/icon/assets/vector_triangle';
import { icon as EuiIconWarning } from '@elastic/eui/es/components/icon/assets/warning';
import { icon as EuiIconWifiSlash } from '@elastic/eui/es/components/icon/assets/wifi_slash';
import { appendIconComponentCache } from '@elastic/eui/es/components/icon/icon';
import { lazy, Suspense } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter, Navigate, Route, Routes } from 'react-router';

import { AppContainer } from './app_container';
import { PageLoadingState } from './components';
import { WorkspacePage } from './pages';

const SigninPage = lazy(() => import('./pages/signin'));
const SignupPage = lazy(() => import('./pages/signup'));
const ActivatePage = lazy(() => import('./pages/activate'));

appendIconComponentCache({
  apps: EuiIconApps,
  arrowDown: EuiIconChevronSingleDown,
  arrowLeft: EuiIconChevronSingleLeft,
  arrowRight: EuiIconChevronSingleRight,
  arrowUp: EuiIconChevronSingleUp,
  arrowStart: EuiIconChevronLimitLeft,
  arrowEnd: EuiIconChevronLimitRight,
  bell: EuiIconBell,
  boxesHorizontal: EuiIconBoxesVertical,
  boxesVertical: EuiIconBoxesVertical,
  calendar: EuiIconCalendar,
  check: EuiIconCheck,
  cheer: EuiIconPopper,
  clock: EuiIconClock,
  comment: EuiIconComment,
  controls: EuiIconControls,
  copy: EuiIconCopy,
  copyClipboard: EuiIconCopyClipboard,
  cross: EuiIconCross,
  cut: EuiIconScissors,
  documentation: EuiIconDocumentation,
  dot: EuiIconDot,
  download: EuiIconDownload,
  empty: EuiIconEmpty,
  email: EuiIconMail,
  exit: EuiIconLogOut,
  eyeClosed: EuiIconEyeSlash,
  expand: EuiIconMaximize,
  expandMini: EuiIconMaximize,
  fullScreen: EuiIconFullScreen,
  fullScreenExit: EuiIconFullScreenExit,
  function: EuiIconFunction,
  gear: EuiIconGear,
  globe: EuiIconGlobe,
  grab: EuiIconDragVertical,
  help: EuiIconHelp,
  home: EuiIconHome,
  importAction: EuiIconDownload,
  inputOutput: EuiIconInputOutput,
  keyboard: EuiIconKeyboard,
  link: EuiIconLink,
  listAdd: EuiIconPlusCircle,
  minus: EuiIconMinus,
  minusInCircle: EuiIconMinusCircle,
  node: EuiIconVectorTriangle,
  offline: EuiIconWifiSlash,
  payment: EuiIconPayment,
  pencil: EuiIconPencil,
  plusInCircle: EuiIconPlusCircle,
  popout: EuiIconExternal,
  question: EuiIconQuestion,
  radar: EuiIconRadar,
  refresh: EuiIconRefresh,
  returnKey: EuiIconReturn,
  search: EuiIconMagnify,
  securityApp: EuiIconSecurityApp,
  securitySignalDetected: EuiIconSecuritySignalDetected,
  share: EuiIconShare,
  sortable: EuiIconSortable,
  sortUp: EuiIconSortUp,
  sortDown: EuiIconSortDown,
  sortRight: EuiIconSortRight,
  sortLeft: EuiIconSortLeft,
  starEmpty: EuiIconStar,
  starFilled: EuiIconStarFill,
  tableDensityCompact: EuiIconTableDensityHigh,
  tableDensityExpanded: EuiIconTableDensityLow,
  tableDensityNormal: EuiIconTable,
  timeRefresh: EuiIconRefreshTime,
  tokenKey: EuiIconTokenKey,
  tokenNumber: EuiIconTokenNumber,
  tokenString: EuiIconTokenString,
  training: EuiIconPresentation,
  trash: EuiIconTrash,
  user: EuiIconUser,
  warning: EuiIconWarning,
});

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
