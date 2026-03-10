import type { SchedulerJobRetryStrategy } from './page_tracker';

export const PAGE_TRACKER_MANUAL_SCHEDULE = '@';
export const PAGE_TRACKER_CUSTOM_SCHEDULE = '@@';
export const PAGE_TRACKER_SCHEDULES = [
  { value: PAGE_TRACKER_MANUAL_SCHEDULE, text: 'Manually' },
  { value: '@hourly', text: 'Hourly' },
  { value: '@daily', text: 'Daily' },
  { value: '@weekly', text: 'Weekly' },
  { value: '@monthly', text: 'Monthly' },
  { value: PAGE_TRACKER_CUSTOM_SCHEDULE, text: 'Custom' },
];

/// Recognizes anchored cron expressions produced by `expand_schedule_preset` and maps them back
/// to their preset alias. Returns `null` for non-matching patterns.
export function detectSchedulePreset(schedule: string): string | null {
  const parts = schedule.split(' ');
  if (parts.length !== 6 || parts[0] !== '0' || parts[4] !== '*') {
    return null;
  }
  const [, min, hour, dom, , dow] = parts;
  const isNum = (s: string) => /^\d+$/.test(s);

  if (hour === '*' && dom === '*' && dow === '*' && isNum(min)) return '@hourly';
  if (dom === '*' && dow === '*' && isNum(min) && isNum(hour)) return '@daily';
  if (dom === '*' && isNum(min) && isNum(hour) && isNum(dow)) return '@weekly';
  if (dow === '*' && isNum(min) && isNum(hour) && isNum(dom)) return '@monthly';
  return null;
}

export function getScheduleMinInterval(schedule: string) {
  const effective = detectSchedulePreset(schedule) ?? schedule;
  switch (effective) {
    case '@hourly':
      return 3600000;
    case '@daily':
      return 86400000;
    case '@weekly':
      return 604800000;
    case '@monthly':
      return 2592000000;
    default:
      return 0;
  }
}

export interface AnchorParams {
  minute: number;
  hour: number;
  weekday: number;
  dayOfMonth: number;
}

export function defaultAnchorParams(): AnchorParams {
  const now = new Date();
  return {
    minute: now.getUTCMinutes(),
    hour: now.getUTCHours(),
    weekday: now.getUTCDay(),
    dayOfMonth: Math.min(now.getUTCDate(), 28),
  };
}

export function buildAnchoredCron(preset: string, params: AnchorParams): string {
  switch (preset) {
    case '@hourly':
      return `0 ${params.minute} * * * *`;
    case '@daily':
      return `0 ${params.minute} ${params.hour} * * *`;
    case '@weekly':
      return `0 ${params.minute} ${params.hour} * * ${params.weekday}`;
    case '@monthly':
      return `0 ${params.minute} ${params.hour} ${params.dayOfMonth} * *`;
    default:
      return preset;
  }
}

export function parseAnchorParams(schedule: string): AnchorParams | null {
  const preset = detectSchedulePreset(schedule);
  if (!preset) {
    return null;
  }

  const [, min, hour, dom, , dow] = schedule.split(' ');
  return {
    minute: parseInt(min, 10),
    hour: hour !== '*' ? parseInt(hour, 10) : 0,
    weekday: dow !== '*' ? parseInt(dow, 10) : 0,
    dayOfMonth: dom !== '*' ? parseInt(dom, 10) : 1,
  };
}

export const WEEKDAY_OPTIONS = [
  { value: '0', text: 'Sunday' },
  { value: '1', text: 'Monday' },
  { value: '2', text: 'Tuesday' },
  { value: '3', text: 'Wednesday' },
  { value: '4', text: 'Thursday' },
  { value: '5', text: 'Friday' },
  { value: '6', text: 'Saturday' },
];

export const HOUR_OPTIONS = Array.from({ length: 24 }, (_, i) => ({
  value: String(i),
  text: String(i).padStart(2, '0'),
}));

export const MINUTE_OPTIONS = Array.from({ length: 60 }, (_, i) => ({
  value: String(i),
  text: String(i).padStart(2, '0'),
}));

export const DAY_OF_MONTH_OPTIONS = Array.from({ length: 28 }, (_, i) => ({
  value: String(i + 1),
  text: String(i + 1),
}));

export function getRetryStrategies(retryIntervals: RetryInterval[]) {
  return [
    { value: 'none', text: 'None' },
    ...(retryIntervals.length > 0 ? [{ value: 'constant', text: 'Constant backoff' }] : []),
  ];
}

export type RetryInterval = { label: string; value: number };
export function getRetryIntervals(minInterval: number): RetryInterval[] {
  if (minInterval > 1209600000 /** 14 days **/) {
    return [
      { label: '3h', value: 10800000 },
      { label: '12h', value: 43200000 },
      { label: '1d', value: 86400000 },
      { label: '2d', value: 172800000 },
      { label: '3d', value: 259200000 },
    ];
  }

  if (minInterval > 172800000 /** 48 hours **/) {
    return [
      { label: '1h', value: 3600000 },
      { label: '3h', value: 10800000 },
      { label: '6h', value: 21600000 },
      { label: '9h', value: 32400000 },
      { label: '12h', value: 43200000 },
    ];
  }

  if (minInterval > 3600000 /** 1 hour **/) {
    return [
      { label: '10m', value: 600000 },
      { label: '30m', value: 1800000 },
      { label: '1h', value: 3600000 },
      { label: '2h', value: 7200000 },
      { label: '3h', value: 10800000 },
    ];
  }

  if (minInterval > 600000 /** 10 minutes **/) {
    return [
      { label: '1m', value: 60000 },
      { label: '3m', value: 180000 },
      { label: '5m', value: 300000 },
      { label: '7m', value: 420000 },
      { label: '10m', value: 600000 },
    ];
  }

  // For intervals less than 10 minutes, it doesn't make sense to retry more than once.
  return [];
}

export function getDefaultRetryStrategy(retryIntervals: RetryInterval[]): SchedulerJobRetryStrategy {
  return { type: 'constant', maxAttempts: 3, interval: getDefaultRetryInterval(retryIntervals) };
}

// By default, use the middle interval, e.g. 5 minutes for hourly schedule.
export function getDefaultRetryInterval(intervals: RetryInterval[]) {
  return intervals[Math.floor(intervals.length / 2)].value;
}
