import { EuiButtonIcon, EuiFieldText, EuiSelect, EuiSpacer, EuiToolTip } from '@elastic/eui';
import { useEffect, useState } from 'react';

import type { RetryInterval } from './consts';
import {
  getRetryIntervals,
  getScheduleMinInterval,
  WEB_PAGE_TRACKER_CUSTOM_SCHEDULE,
  WEB_PAGE_TRACKER_MANUAL_SCHEDULE,
  WEB_PAGE_TRACKER_SCHEDULES,
} from './consts';
import type { AsyncData } from '../../../../model';
import { getErrorMessage } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

export interface WebPageTrackerJobScheduleProps {
  schedule?: string;
  onChange: (schedule: string | null, retryIntervals: RetryInterval[]) => void;
}

interface CustomScheduleCheck {
  minInterval: number;
  nextOccurrences: number[];
}

export function WebPageTrackerJobSchedule({ schedule, onChange }: WebPageTrackerJobScheduleProps) {
  const { uiState } = useWorkspaceContext();

  // Filter schedules based on subscription.
  const subscriptionSchedules = uiState.subscription?.features?.webScraping.trackerSchedules;
  const schedules = subscriptionSchedules
    ? WEB_PAGE_TRACKER_SCHEDULES.filter((knownSchedule) => subscriptionSchedules.includes(knownSchedule.value))
    : WEB_PAGE_TRACKER_SCHEDULES;
  const [scheduleType, setScheduleType] = useState<string>(() => {
    if (!schedule) {
      return WEB_PAGE_TRACKER_MANUAL_SCHEDULE;
    }

    return schedules.some((s) => s.value === schedule) ? schedule : WEB_PAGE_TRACKER_CUSTOM_SCHEDULE;
  });

  const [customSchedule, setCustomSchedule] = useState<string>(
    scheduleType === WEB_PAGE_TRACKER_CUSTOM_SCHEDULE ? (schedule ?? '') : '',
  );
  const [customScheduleCheck, setCustomScheduleCheck] = useState<AsyncData<CustomScheduleCheck> | null>(null);
  useEffect(() => {
    if (!customSchedule) {
      setCustomScheduleCheck(null);
      return;
    }

    setCustomScheduleCheck({ status: 'pending' });

    fetch(`/api/scheduler/parse_schedule`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ schedule: customSchedule }),
    })
      .then((response) => response.json())
      .then((data: CustomScheduleCheck) => {
        setCustomScheduleCheck({ status: 'succeeded', data });
        onChange(customSchedule, getRetryIntervals(data.minInterval));
      })
      .catch((e) => {
        setCustomScheduleCheck({ status: 'failed', error: getErrorMessage(e) });
        onChange('', []);
      });
  }, [customSchedule]);

  const typePicker = (
    <EuiSelect
      options={schedules}
      value={scheduleType}
      onChange={(e) => {
        setScheduleType(e.target.value);
        setCustomSchedule('');

        if (
          e.target.value === WEB_PAGE_TRACKER_MANUAL_SCHEDULE ||
          e.target.value === WEB_PAGE_TRACKER_CUSTOM_SCHEDULE
        ) {
          onChange(null, []);
        } else {
          onChange(e.target.value, getRetryIntervals(getScheduleMinInterval(e.target.value)));
        }
      }}
    />
  );

  if (scheduleType !== WEB_PAGE_TRACKER_CUSTOM_SCHEDULE) {
    return typePicker;
  }

  return (
    <div>
      {typePicker}
      <EuiSpacer size={'s'} />
      <EuiFieldText
        placeholder={'Cron expression, e.g., 0 30 9 * * Mon,Wed,Sat'}
        value={customSchedule}
        isLoading={customScheduleCheck?.status === 'pending'}
        isInvalid={customScheduleCheck?.status === 'failed'}
        append={
          customScheduleCheck?.status === 'succeeded' ? (
            <EuiToolTip
              title="Upcoming checks"
              position={'bottom'}
              content={
                <>
                  {customScheduleCheck.data.nextOccurrences.map((occurrence) => (
                    <p key={occurrence}>{new Date(occurrence * 1000).toUTCString()}</p>
                  ))}
                </>
              }
            >
              <EuiButtonIcon iconType={'calendar'} aria-label="Show next occurences" />
            </EuiToolTip>
          ) : (
            <EuiButtonIcon iconType={'calendar'} aria-label="Next occurences aren't available" isDisabled />
          )
        }
        onChange={(e) => setCustomSchedule(e.target.value)}
      />
    </div>
  );
}
