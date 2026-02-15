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
 * Converts revisions to chart data points.
 * Returns data sorted by timestamp (oldest first) for proper chart display.
 */
export function revisionsToChartData(revisions: TrackerDataRevision[]): ChartDataPoint[] {
  return revisions
    .map((revision) => ({
      // Convert to milliseconds
      timestamp: revision.createdAt * 1000,
      value: parseNumericValue(revision.data.original),
      formattedDate: new Date(revision.createdAt * 1000).toLocaleString(),
    }))
    .sort((a, b) => a.timestamp - b.timestamp);
}

/**
 * Formats a number for display, handling various magnitudes.
 */
export function formatChartValue(value: number): string {
  if (Number.isInteger(value)) {
    return value.toLocaleString();
  }

  // For decimals, show up to 4 significant decimal places
  const absValue = Math.abs(value);
  if (absValue >= 1000) {
    return value.toLocaleString(undefined, { maximumFractionDigits: 2 });
  }

  if (absValue >= 1) {
    return value.toLocaleString(undefined, { maximumFractionDigits: 4 });
  }

  // For very small numbers, show more precision
  return value.toLocaleString(undefined, { maximumSignificantDigits: 4 });
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
