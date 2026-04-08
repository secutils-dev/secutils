import type { TrackerDataRevision } from '../tracker_data_revision';

/**
 * Represents a data point for the chart view.
 */
export interface ChartDataPoint {
  timestamp: number;
  value: number;
  formattedDate: string;
}

/**
 * Checks if all revisions contain chartable (numeric) data.
 * Requires at least 2 revisions for a meaningful chart.
 */
export function isChartableData(revisions: TrackerDataRevision[]) {
  if (revisions.length < 2) {
    return false;
  }

  return revisions.every((revision) => isChartableRevisionData(revision.data.original));
}

/**
 * Checks if a single value is numeric (number or numeric string).
 */
function isNumericValue(value: unknown) {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return true;
  }

  if (typeof value === 'string') {
    const trimmed = value.trim();
    if (trimmed === '') {
      return false;
    }

    // Handle numbers with commas (e.g., "1,234.56" or "1.234,56")
    const normalized = trimmed.replace(/,/g, '');
    const parsed = Number(normalized);
    return Number.isFinite(parsed);
  }

  return false;
}

/**
 * Checks if a revision's data is chartable (numeric scalar value).
 */
function isChartableRevisionData(data: unknown) {
  return isNumericValue(data);
}

/**
 * Extracts the millisecond-precision Unix timestamp from a UUIDv7 string.
 * UUIDv7 encodes the timestamp in the first 48 bits (12 hex characters).
 */
function uuidv7ToTimestamp(uuid: string): number {
  const hex = uuid.replace(/-/g, '').substring(0, 12);
  return parseInt(hex, 16);
}

/**
 * Converts revisions to chart data points.
 * Returns data sorted by timestamp (oldest first) for proper chart display.
 * Uses the millisecond-precision timestamp from UUIDv7 IDs so that revisions
 * created within the same second get distinct X-axis positions.
 */
export function revisionsToChartData(revisions: TrackerDataRevision[]): ChartDataPoint[] {
  return [...revisions]
    .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
    .map((revision) => {
      const timestamp = uuidv7ToTimestamp(revision.id);
      return {
        timestamp,
        value: parseNumericValue(revision.data.original),
        formattedDate: new Date(timestamp).toLocaleString(),
      };
    });
}

/**
 * Formats a number for display, handling various magnitudes.
 */
export function formatChartValue(value: number): string {
  if (Number.isInteger(value)) {
    return value.toLocaleString();
  }

  // For decimals, show up to 6 significant decimal places
  const absValue = Math.abs(value);
  if (absValue >= 1000) {
    return value.toLocaleString(undefined, { maximumFractionDigits: 4 });
  }

  if (absValue >= 1) {
    return value.toLocaleString(undefined, { maximumFractionDigits: 6 });
  }

  // For very small numbers, show more precision
  return value.toLocaleString(undefined, { maximumSignificantDigits: 6 });
}

const COMPACT_SUFFIXES: Array<[number, string]> = [
  [1e12, 'T'],
  [1e9, 'B'],
  [1e6, 'M'],
  [1e3, 'K'],
];

/**
 * Formats a number compactly for Y-axis tick labels (e.g., "1.78T", "234K").
 * Falls back to full formatting for numbers below 1,000.
 */
export function formatCompactValue(value: number): string {
  const absValue = Math.abs(value);
  for (const [threshold, suffix] of COMPACT_SUFFIXES) {
    if (absValue >= threshold) {
      const scaled = value / threshold;
      const formatted = scaled.toLocaleString(undefined, {
        minimumFractionDigits: 0,
        maximumFractionDigits: 2,
      });
      return `${formatted}${suffix}`;
    }
  }
  return formatChartValue(value);
}

/**
 * Parses a value to a number.
 */
function parseNumericValue(value: unknown) {
  if (typeof value === 'number') {
    return value;
  }

  if (typeof value === 'string') {
    const normalized = value.trim().replace(/,/g, '');
    return Number(normalized);
  }

  return NaN;
}
