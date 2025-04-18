import { PageLoadingState } from './page_loading_state';

export function PageUnderConstructionState() {
  return <PageLoadingState title={`🚧 This functionality is not yet implemented…`} />;
}

export default PageUnderConstructionState;
