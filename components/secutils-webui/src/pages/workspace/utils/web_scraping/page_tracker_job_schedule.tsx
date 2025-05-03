import { EuiButtonIcon, EuiFieldText, EuiSelect, EuiSpacer, EuiToolTip } from '@elastic/eui';
import { useEffect, useState } from 'react';

import type { RetryInterval } from './consts';
import {
  getRetryIntervals,
  getScheduleMinInterval,
  PAGE_TRACKER_CUSTOM_SCHEDULE,
  PAGE_TRACKER_MANUAL_SCHEDULE,
  PAGE_TRACKER_SCHEDULES,
} from './consts';
import type { AsyncData } from '../../../../model';
import { getErrorMessage } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

export interface PageTrackerJobScheduleProps {
  schedule?: string;
  onChange: (schedule: string | null, retryIntervals: RetryInterval[]) => void;
}

interface CustomScheduleCheck {
  minInterval: number;
  nextOccurrences: number[];
}

export function PageTrackerJobSchedule({ schedule, onChange }: PageTrackerJobScheduleProps) {
  const { uiState } = useWorkspaceContext();

  // Filter schedules based on subscription.
  const subscriptionSchedules = uiState.subscription?.features?.webScraping.trackerSchedules;
  const schedules = subscriptionSchedules
    ? PAGE_TRACKER_SCHEDULES.filter((knownSchedule) => subscriptionSchedules.includes(knownSchedule.value))
    : PAGE_TRACKER_SCHEDULES;
  const [scheduleType, setScheduleType] = useState<string>(() => {
    if (!schedule) {
      return PAGE_TRACKER_MANUAL_SCHEDULE;
    }

    return schedules.some((s) => s.value === schedule) ? schedule : PAGE_TRACKER_CUSTOM_SCHEDULE;
  });

  const [customSchedule, setCustomSchedule] = useState<string>(
    scheduleType === PAGE_TRACKER_CUSTOM_SCHEDULE ? (schedule ?? '') : '',
  );
  const [customScheduleValidated, setCustomScheduleValidated] = useState<boolean>(false);
  const [customScheduleCheck, setCustomScheduleCheck] = useState<AsyncData<CustomScheduleCheck> | null>(null);
  useEffect(() => {
    if (!customSchedule) {
      setCustomScheduleCheck(null);
      return;
    }

    if (customScheduleValidated) {
      // If the custom schedule is already validated, we don't need to re-validate it.
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
        setCustomScheduleValidated(true);
        onChange(customSchedule, getRetryIntervals(data.minInterval));
      })
      .catch((e) => {
        setCustomScheduleCheck({ status: 'failed', error: getErrorMessage(e) });
        setCustomScheduleValidated(true);
        onChange('', []);
      });
  }, [customSchedule, customScheduleValidated, onChange]);

  const typePicker = (
    <EuiSelect
      options={schedules}
      value={scheduleType}
      onChange={(e) => {
        setScheduleType(e.target.value);
        setCustomSchedule('');

        if (e.target.value === PAGE_TRACKER_MANUAL_SCHEDULE || e.target.value === PAGE_TRACKER_CUSTOM_SCHEDULE) {
          onChange(null, []);
        } else {
          onChange(e.target.value, getRetryIntervals(getScheduleMinInterval(e.target.value)));
        }
      }}
    />
  );

  if (scheduleType !== PAGE_TRACKER_CUSTOM_SCHEDULE) {
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
        onChange={(e) => {
          setCustomSchedule(e.target.value);
          setCustomScheduleValidated(false);
        }}
      />
    </div>
  );
}
