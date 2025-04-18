import { lazy, Suspense } from 'react';
import { BrowserRouter, Navigate, Route, Routes } from 'react-router';
import { createRoot } from 'react-dom/client';

import './index.css';

import { icon as EuiIconSecurityApp } from '@elastic/eui/es/components/icon/assets/app_security';
import { icon as EuiIconApps } from '@elastic/eui/es/components/icon/assets/apps';
import { icon as EuiIconArrowDown } from '@elastic/eui/es/components/icon/assets/arrow_down';
import { icon as EuiIconArrowLeft } from '@elastic/eui/es/components/icon/assets/arrow_left';
import { icon as EuiIconArrowRight } from '@elastic/eui/es/components/icon/assets/arrow_right';
import { icon as EuiIconArrowUp } from '@elastic/eui/es/components/icon/assets/arrow_up';
import { icon as EuiIconArrowEnd } from '@elastic/eui/es/components/icon/assets/arrowEnd';
import { icon as EuiIconArrowStart } from '@elastic/eui/es/components/icon/assets/arrowStart';
import { icon as EuiIconBell } from '@elastic/eui/es/components/icon/assets/bell';
import { icon as EuiIconBoxesHorizontal } from '@elastic/eui/es/components/icon/assets/boxes_horizontal';
import { icon as EuiIconBoxesVertical } from '@elastic/eui/es/components/icon/assets/boxes_vertical';
import { icon as EuiIconCalendar } from '@elastic/eui/es/components/icon/assets/calendar';
import { icon as EuiIconCheck } from '@elastic/eui/es/components/icon/assets/check';
import { icon as EuiIconCheer } from '@elastic/eui/es/components/icon/assets/cheer';
import { icon as EuiIconClock } from '@elastic/eui/es/components/icon/assets/clock';
import { icon as EuiControlsHorizontal } from '@elastic/eui/es/components/icon/assets/controls_horizontal';
import { icon as EuiIconCopy } from '@elastic/eui/es/components/icon/assets/copy';
import { icon as EuiIconCopyClipboard } from '@elastic/eui/es/components/icon/assets/copy_clipboard';
import { icon as EuiIconCross } from '@elastic/eui/es/components/icon/assets/cross';
import { icon as EuiIconCut } from '@elastic/eui/es/components/icon/assets/cut';
import { icon as EuiIconDiscuss } from '@elastic/eui/es/components/icon/assets/discuss';
import { icon as EuiIconDocumentation } from '@elastic/eui/es/components/icon/assets/documentation';
import { icon as EuiIconDot } from '@elastic/eui/es/components/icon/assets/dot';
import { icon as EuiIconDownload } from '@elastic/eui/es/components/icon/assets/download';
import { icon as EuiIconEmail } from '@elastic/eui/es/components/icon/assets/email';
import { icon as EuiIconEmpty } from '@elastic/eui/es/components/icon/assets/empty';
import { icon as EuiIconExit } from '@elastic/eui/es/components/icon/assets/exit';
import { icon as EuiIconExpandMini } from '@elastic/eui/es/components/icon/assets/expandMini';
import { icon as EuiIconEyeClosed } from '@elastic/eui/es/components/icon/assets/eye_closed';
import { icon as EuiIconFullScreen } from '@elastic/eui/es/components/icon/assets/full_screen';
import { icon as EuiIconFullScreenExit } from '@elastic/eui/es/components/icon/assets/fullScreenExit';
import { icon as EuiIconFunction } from '@elastic/eui/es/components/icon/assets/function';
import { icon as EuiIconGear } from '@elastic/eui/es/components/icon/assets/gear';
import { icon as EuiIconGlobe } from '@elastic/eui/es/components/icon/assets/globe';
import { icon as EuiIconGrab } from '@elastic/eui/es/components/icon/assets/grab';
import { icon as EuiIconHelp } from '@elastic/eui/es/components/icon/assets/help';
import { icon as EuiIconHome } from '@elastic/eui/es/components/icon/assets/home';
import { icon as EuiIconImport } from '@elastic/eui/es/components/icon/assets/import';
import { icon as EuiIconInputOutput } from '@elastic/eui/es/components/icon/assets/inputOutput';
import { icon as EuiIconKeyboard } from '@elastic/eui/es/components/icon/assets/keyboard';
import { icon as EuiIconLink } from '@elastic/eui/es/components/icon/assets/link';
import { icon as EuiIconListAdd } from '@elastic/eui/es/components/icon/assets/list_add';
import { icon as EuiIconMinus } from '@elastic/eui/es/components/icon/assets/minus';
import { icon as EuiIconMinusInCircle } from '@elastic/eui/es/components/icon/assets/minus_in_circle';
import { icon as EuiIconNode } from '@elastic/eui/es/components/icon/assets/node';
import { icon as EuiIconOffline } from '@elastic/eui/es/components/icon/assets/offline';
import { icon as EuiIconPayment } from '@elastic/eui/es/components/icon/assets/payment';
import { icon as EuiIconPencil } from '@elastic/eui/es/components/icon/assets/pencil';
import { icon as EuiIconPlusInCircle } from '@elastic/eui/es/components/icon/assets/plus_in_circle';
import { icon as EuiIconPopout } from '@elastic/eui/es/components/icon/assets/popout';
import { icon as EuiIconQuestionInCircle } from '@elastic/eui/es/components/icon/assets/question_in_circle';
import { icon as EuiIconRefresh } from '@elastic/eui/es/components/icon/assets/refresh';
import { icon as EuiIconReturnKey } from '@elastic/eui/es/components/icon/assets/return_key';
import { icon as EuiIconSearch } from '@elastic/eui/es/components/icon/assets/search';
import { icon as EuiIconSecuritySignal } from '@elastic/eui/es/components/icon/assets/securitySignal';
import { icon as EuiIconSecuritySignalDetected } from '@elastic/eui/es/components/icon/assets/securitySignalDetected';
import { icon as EuiIconShare } from '@elastic/eui/es/components/icon/assets/share';
import { icon as EuiIconSortDown } from '@elastic/eui/es/components/icon/assets/sort_down';
import { icon as EuiIconSortUp } from '@elastic/eui/es/components/icon/assets/sort_up';
import { icon as EuiIconSortable } from '@elastic/eui/es/components/icon/assets/sortable';
import { icon as EuiIconSortLeft } from '@elastic/eui/es/components/icon/assets/sortLeft';
import { icon as EuiIconSortRight } from '@elastic/eui/es/components/icon/assets/sortRight';
import { icon as EuiIconStarEmpty } from '@elastic/eui/es/components/icon/assets/star_empty';
import { icon as EuiIconStarFilled } from '@elastic/eui/es/components/icon/assets/star_filled';
import { icon as EuiIconTableDensityCompact } from '@elastic/eui/es/components/icon/assets/table_density_compact';
import { icon as EuiIconTableDensityExpanded } from '@elastic/eui/es/components/icon/assets/table_density_expanded';
import { icon as EuiIconTableDensityNormal } from '@elastic/eui/es/components/icon/assets/table_density_normal';
import { icon as EuiIconTimeRefresh } from '@elastic/eui/es/components/icon/assets/timeRefresh';
import { icon as EuiIconTokenNumber } from '@elastic/eui/es/components/icon/assets/tokenNumber';
import { icon as EuiIconTokenString } from '@elastic/eui/es/components/icon/assets/tokenString';
import { icon as EuiIconTraining } from '@elastic/eui/es/components/icon/assets/training';
import { icon as EuiIconTrash } from '@elastic/eui/es/components/icon/assets/trash';
import { icon as EuiIconUser } from '@elastic/eui/es/components/icon/assets/user';
import { icon as EuiIconWarning } from '@elastic/eui/es/components/icon/assets/warning';
import { appendIconComponentCache } from '@elastic/eui/es/components/icon/icon';

import { AppContainer } from './app_container';
import { PageLoadingState } from './components';
import { WorkspacePage } from './pages';

const SigninPage = lazy(() => import('./pages/signin'));
const SignupPage = lazy(() => import('./pages/signup'));
const ActivatePage = lazy(() => import('./pages/activate'));

appendIconComponentCache({
  apps: EuiIconApps,
  arrowDown: EuiIconArrowDown,
  arrowLeft: EuiIconArrowLeft,
  arrowRight: EuiIconArrowRight,
  arrowUp: EuiIconArrowUp,
  arrowStart: EuiIconArrowStart,
  arrowEnd: EuiIconArrowEnd,
  bell: EuiIconBell,
  boxesHorizontal: EuiIconBoxesHorizontal,
  boxesVertical: EuiIconBoxesVertical,
  calendar: EuiIconCalendar,
  check: EuiIconCheck,
  cheer: EuiIconCheer,
  clock: EuiIconClock,
  controlsHorizontal: EuiControlsHorizontal,
  copy: EuiIconCopy,
  copyClipboard: EuiIconCopyClipboard,
  cross: EuiIconCross,
  cut: EuiIconCut,
  discuss: EuiIconDiscuss,
  documentation: EuiIconDocumentation,
  dot: EuiIconDot,
  download: EuiIconDownload,
  empty: EuiIconEmpty,
  email: EuiIconEmail,
  exit: EuiIconExit,
  eyeClosed: EuiIconEyeClosed,
  expandMini: EuiIconExpandMini,
  fullScreen: EuiIconFullScreen,
  fullScreenExit: EuiIconFullScreenExit,
  function: EuiIconFunction,
  gear: EuiIconGear,
  globe: EuiIconGlobe,
  grab: EuiIconGrab,
  help: EuiIconHelp,
  home: EuiIconHome,
  importAction: EuiIconImport,
  inputOutput: EuiIconInputOutput,
  keyboard: EuiIconKeyboard,
  link: EuiIconLink,
  listAdd: EuiIconListAdd,
  minus: EuiIconMinus,
  minusInCircle: EuiIconMinusInCircle,
  node: EuiIconNode,
  offline: EuiIconOffline,
  payment: EuiIconPayment,
  pencil: EuiIconPencil,
  plusInCircle: EuiIconPlusInCircle,
  popout: EuiIconPopout,
  questionInCircle: EuiIconQuestionInCircle,
  refresh: EuiIconRefresh,
  returnKey: EuiIconReturnKey,
  search: EuiIconSearch,
  securityApp: EuiIconSecurityApp,
  securitySignal: EuiIconSecuritySignal,
  securitySignalDetected: EuiIconSecuritySignalDetected,
  share: EuiIconShare,
  sortable: EuiIconSortable,
  sortUp: EuiIconSortUp,
  sortDown: EuiIconSortDown,
  sortRight: EuiIconSortRight,
  sortLeft: EuiIconSortLeft,
  starEmpty: EuiIconStarEmpty,
  starFilled: EuiIconStarFilled,
  tableDensityCompact: EuiIconTableDensityCompact,
  tableDensityExpanded: EuiIconTableDensityExpanded,
  tableDensityNormal: EuiIconTableDensityNormal,
  timeRefresh: EuiIconTimeRefresh,
  tokenNumber: EuiIconTokenNumber,
  tokenString: EuiIconTokenString,
  training: EuiIconTraining,
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
