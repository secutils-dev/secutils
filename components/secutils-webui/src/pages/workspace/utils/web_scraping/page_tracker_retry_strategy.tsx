import { EuiFormRow, EuiRange, EuiSelect } from '@elastic/eui';

import type { RetryInterval } from './consts';
import { getDefaultRetryStrategy, getRetryStrategies } from './consts';
import type { SchedulerJobRetryStrategy } from './page_tracker';

export interface PageTrackerRetryStrategyProps {
  intervals: RetryInterval[];
  strategy?: SchedulerJobRetryStrategy;
  onChange: (strategy: SchedulerJobRetryStrategy | null) => void;
}

export function PageTrackerRetryStrategy({ intervals, strategy, onChange }: PageTrackerRetryStrategyProps) {
  let maxAttempts = null;
  let interval = null;
  if (strategy && intervals.length > 0) {
    maxAttempts = (
      <EuiFormRow label="Attemtps" helpText="How many retries should be attempted if check fails">
        <EuiRange
          min={1}
          max={10}
          step={1}
          value={strategy.maxAttempts}
          onChange={(e) => onChange({ ...strategy, maxAttempts: +e.currentTarget.value })}
          showTicks
        />
      </EuiFormRow>
    );

    const minInterval = intervals[0].value;
    const maxInterval = intervals[intervals.length - 1].value;
    interval = (
      <EuiFormRow label="Interval" helpText="How long to wait between retries if check attempt fails">
        <EuiRange
          min={minInterval}
          max={maxInterval}
          step={minInterval}
          value={strategy.interval}
          disabled={strategy.maxAttempts === 0}
          ticks={intervals}
          onChange={(e) => onChange({ ...strategy, interval: +e.currentTarget.value })}
          showTicks
        />
      </EuiFormRow>
    );
  }

  const strategies = getRetryStrategies(intervals);
  const canChangeStrategy = strategies.length > 1;
  return (
    <>
      <EuiFormRow label="Strategy" helpText="What strategy should be used to retry failed checks">
        <EuiSelect
          options={strategies}
          disabled={!canChangeStrategy}
          value={strategy?.type ?? strategies[0].value}
          onChange={
            canChangeStrategy
              ? (e) => onChange(e.target.value === 'none' ? null : getDefaultRetryStrategy(intervals))
              : undefined
          }
        />
      </EuiFormRow>
      {maxAttempts}
      {interval}
    </>
  );
}
