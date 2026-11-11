// Browser stub -- grid debug logger is a no-op (debug-only path in upstream).
// Lifted from run-llama/liteparse's browser-compat stubs.
export interface GridDebugConfig {
  enabled: boolean;
}
export const DEFAULT_DEBUG_CONFIG = { enabled: false };
export interface RenderedSegment {
  lineIndex: number;
  gridCol: number;
  text: string;
  snap: string;
}
export interface RenderTraceContext {}
export interface VisualizerPageData {
  pageNum: number;
  segments: RenderedSegment[];
  rawLines: string[];
}

export class GridDebugLogger {
  config = { enabled: false };
  get enabled() {
    return false;
  }
  get shouldVisualize() {
    return false;
  }
  get visualizerPages(): VisualizerPageData[] {
    return [];
  }
  get debugConfig() {
    return this.config;
  }
  matchesBbox() {
    return false;
  }
  setPage() {}
  logBlock() {}
  logFlowingBlock() {}
  logStructuredBlock() {}
  logFlowingLine() {}
  logAnchors() {}
  logSnapAssignment() {}
  captureRender() {}
  captureRawLines() {}
  logRender() {}
  logForwardAnchor() {}
  logDuplicateResolution() {}
  logBlockContext() {}
  logRenderTrace() {}
  logForwardAnchorMutation() {}
  logLineComposition() {}
  flushSync() {}
  async flush() {}
}

export const NOOP_LOGGER = new GridDebugLogger();
export function createGridDebugLogger() {
  return NOOP_LOGGER;
}
