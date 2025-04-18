import type { SchedulerJobRetryStrategy } from './web_page_tracker';

export const WEB_PAGE_TRACKER_MANUAL_SCHEDULE = '@';
export const WEB_PAGE_TRACKER_CUSTOM_SCHEDULE = '@@';
export const WEB_PAGE_TRACKER_SCHEDULES = [
  { value: WEB_PAGE_TRACKER_MANUAL_SCHEDULE, text: 'Manually' },
  { value: '@hourly', text: 'Hourly' },
  { value: '@daily', text: 'Daily' },
  { value: '@weekly', text: 'Weekly' },
  { value: '@monthly', text: 'Monthly' },
  { value: WEB_PAGE_TRACKER_CUSTOM_SCHEDULE, text: 'Custom' },
];

export function getScheduleMinInterval(schedule: string) {
  switch (schedule) {
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
