/**
 * Curated type stubs for the Page Tracker content extractor script editor.
 * Registered via Monaco's `addExtraLib` to provide IntelliSense for Playwright's
 * Page API, the extractor context parameter, and the Retrack resource utilities.
 */
export const PAGE_TRACKER_TYPE_DEFS = `
/** Playwright Response object returned by navigation and network events. */
interface Response {
  /** Response URL. */
  url(): string;
  /** Response status code. */
  status(): number;
  /** Whether the response status was successful (2xx). */
  ok(): boolean;
  /** Response body as text. */
  text(): Promise<string>;
  /** Response body parsed as JSON. */
  json(): Promise<any>;
  /** Response headers. */
  headers(): Record<string, string>;
}

/** Playwright ElementHandle — a handle to a DOM element. */
interface ElementHandle {
  /** Returns the \`element.textContent\`. */
  textContent(): Promise<string | null>;
  /** Returns the \`element.innerHTML\`. */
  innerHTML(): Promise<string>;
  /** Clicks the element. */
  click(options?: { button?: 'left' | 'right' | 'middle'; delay?: number }): Promise<void>;
  /** Types text into the element (input, textarea). */
  fill(value: string): Promise<void>;
  /** Returns an attribute value. */
  getAttribute(name: string): Promise<string | null>;
  /** Returns whether the element is visible. */
  isVisible(): Promise<boolean>;
  /** Waits for the element to satisfy a state. */
  waitForElementState(state: 'visible' | 'hidden' | 'stable' | 'enabled' | 'disabled'): Promise<void>;
  /** Returns element's bounding box. */
  boundingBox(): Promise<{ x: number; y: number; width: number; height: number } | null>;
  /** Queries a descendant matching the selector. */
  $(selector: string): Promise<ElementHandle | null>;
  /** Queries all descendants matching the selector. */
  $$(selector: string): Promise<ElementHandle[]>;
  /** Evaluates a function in the element's context. */
  evaluate<T = any>(pageFunction: (element: Element, ...args: any[]) => T, ...args: any[]): Promise<T>;
}

/** Playwright Locator — a way to find element(s) on the page at any moment. */
interface Locator {
  /** Click the element. */
  click(options?: { button?: 'left' | 'right' | 'middle'; delay?: number; timeout?: number }): Promise<void>;
  /** Fill an input or textarea. */
  fill(value: string, options?: { timeout?: number }): Promise<void>;
  /** Returns the text content. */
  textContent(options?: { timeout?: number }): Promise<string | null>;
  /** Returns the inner HTML. */
  innerHTML(options?: { timeout?: number }): Promise<string>;
  /** Returns the inner text. */
  innerText(options?: { timeout?: number }): Promise<string>;
  /** Returns an attribute value. */
  getAttribute(name: string, options?: { timeout?: number }): Promise<string | null>;
  /** Returns the input value. */
  inputValue(options?: { timeout?: number }): Promise<string>;
  /** Whether the element is visible. */
  isVisible(options?: { timeout?: number }): Promise<boolean>;
  /** Whether the element is enabled. */
  isEnabled(options?: { timeout?: number }): Promise<boolean>;
  /** Wait for the element to be in a particular state. */
  waitFor(options?: { state?: 'attached' | 'detached' | 'visible' | 'hidden'; timeout?: number }): Promise<void>;
  /** Returns the number of elements matching the locator. */
  count(): Promise<number>;
  /** Returns the first matching element. */
  first(): Locator;
  /** Returns the last matching element. */
  last(): Locator;
  /** Returns the nth matching element (0-indexed). */
  nth(index: number): Locator;
  /** Narrows to a descendant locator. */
  locator(selectorOrLocator: string | Locator): Locator;
  /** Evaluates a function in the element's context. */
  evaluate<T = any>(pageFunction: (element: Element, ...args: any[]) => T, ...args: any[]): Promise<T>;
}

/** Playwright Page — the main object for interacting with a browser tab. */
interface Page {
  /** Navigate to a URL. */
  goto(url: string, options?: {
    timeout?: number;
    waitUntil?: 'load' | 'domcontentloaded' | 'networkidle' | 'commit';
  }): Promise<Response | null>;

  /** Returns the page title. */
  title(): Promise<string>;

  /** Returns the full HTML content of the page. */
  content(): Promise<string>;

  /** Returns the current page URL. */
  url(): string;

  /** Execute a function in the browser context and return the result. */
  evaluate<T = any>(pageFunction: ((...args: any[]) => T) | string, ...args: any[]): Promise<T>;

  /** Execute a function in the browser context and return a JSHandle. */
  evaluateHandle(pageFunction: ((...args: any[]) => any) | string, ...args: any[]): Promise<any>;

  /** Returns a Locator for the given CSS or text selector. */
  locator(selector: string): Locator;

  /** Find an element matching the selector, or \`null\`. */
  $(selector: string): Promise<ElementHandle | null>;

  /** Find all elements matching the selector. */
  $$(selector: string): Promise<ElementHandle[]>;

  /** Wait for an element matching the selector to appear. */
  waitForSelector(selector: string, options?: {
    state?: 'attached' | 'detached' | 'visible' | 'hidden';
    timeout?: number;
  }): Promise<ElementHandle | null>;

  /** Wait for the page to reach a load state. */
  waitForLoadState(state?: 'load' | 'domcontentloaded' | 'networkidle', options?: {
    timeout?: number;
  }): Promise<void>;

  /** Wait for the page URL to match. */
  waitForURL(url: string | RegExp | ((url: URL) => boolean), options?: {
    timeout?: number;
    waitUntil?: 'load' | 'domcontentloaded' | 'networkidle' | 'commit';
  }): Promise<void>;

  /** Wait for the given number of milliseconds. */
  waitForTimeout(timeout: number): Promise<void>;

  /** Wait for a network response matching the URL or predicate. */
  waitForResponse(
    urlOrPredicate: string | RegExp | ((response: Response) => boolean | Promise<boolean>),
    options?: { timeout?: number }
  ): Promise<Response>;

  /** Click an element matching the selector. */
  click(selector: string, options?: {
    button?: 'left' | 'right' | 'middle';
    delay?: number;
    timeout?: number;
  }): Promise<void>;

  /** Fill an input or textarea matching the selector. */
  fill(selector: string, value: string, options?: { timeout?: number }): Promise<void>;

  /** Returns \`element.textContent\` for the first element matching the selector. */
  textContent(selector: string, options?: { timeout?: number }): Promise<string | null>;

  /** Returns \`element.innerHTML\` for the first element matching the selector. */
  innerHTML(selector: string, options?: { timeout?: number }): Promise<string>;

  /** Subscribe to a page event (e.g. 'response', 'console', 'dialog'). */
  on(event: string, handler: (...args: any[]) => void): void;

  /** Unsubscribe from a page event. */
  off(event: string, handler: (...args: any[]) => void): void;

  /** Take a screenshot of the page. */
  screenshot(options?: {
    path?: string;
    type?: 'png' | 'jpeg';
    fullPage?: boolean;
    clip?: { x: number; y: number; width: number; height: number };
  }): Promise<Uint8Array>;

  /** Set extra HTTP headers for all requests. */
  setExtraHTTPHeaders(headers: Record<string, string>): Promise<void>;

  /** Reload the current page. */
  reload(options?: {
    timeout?: number;
    waitUntil?: 'load' | 'domcontentloaded' | 'networkidle' | 'commit';
  }): Promise<Response | null>;

  /** Go back in navigation history. */
  goBack(options?: { timeout?: number; waitUntil?: 'load' | 'domcontentloaded' | 'networkidle' }): Promise<Response | null>;

  /** Go forward in navigation history. */
  goForward(options?: { timeout?: number; waitUntil?: 'load' | 'domcontentloaded' | 'networkidle' }): Promise<Response | null>;
}

// --- Retrack resource tracking utilities ---

interface WebPageResourceContentData {
  raw?: string;
  tlsh?: string;
  sha1?: string;
}

interface WebPageResourceContent {
  data: WebPageResourceContentData;
  size: number;
}

interface WebPageResource {
  type: 'script' | 'stylesheet';
  url?: string;
  diff?: 'added' | 'removed' | 'modified';
  content: WebPageResourceContent;
}

interface WebPageResources {
  scripts: WebPageResource[];
  styles: WebPageResource[];
}

interface WebPageResourcesTable {
  '@secutils.data.view': 'table';
  columns: Array<{ id: string; label: string; sortable?: boolean }>;
  rows: Array<Record<string, string | { value: string; color?: string } | undefined>>;
  source: WebPageResources;
}

/** Retrack resource tracking utilities (loaded from https://secutils.dev/retrack/utilities.js). */
declare const utils: {
  /** Start tracking network responses for scripts and stylesheets. Call before page.goto(). */
  startTracking(page: Page): void;
  /** Stop tracking and return collected resources with content digests. Call after the page has loaded. */
  stopTracking(page: Page, maxSizeBytes?: number): Promise<WebPageResources>;
  /** Compare two resource snapshots and annotate diff status (added / removed / modified). */
  setDiffStatus(previous: WebPageResources, current: WebPageResources): WebPageResources;
  /** Format resources into a table view suitable for the Secutils.dev UI. */
  formatAsTable(resources: WebPageResources): WebPageResourcesTable;
};

// --- Content extractor function signature ---

/** Context passed as the second argument to the content extractor function. */
interface ExtractorContext {
  /** Content extracted during the previous execution, if any. */
  previousContent?: {
    /** The raw value returned by the previous execution. */
    original: unknown;
  };
}

/**
 * Content extractor entry point. Implement this function to extract content from a page.
 * @param page - Playwright Page object for browser interaction.
 * @param context - Extraction context with optional previous content.
 * @returns The extracted content (string, object, number, etc.). Must be JSON-serializable.
 */
declare function execute(page: Page, context: ExtractorContext): Promise<unknown>;
`;
