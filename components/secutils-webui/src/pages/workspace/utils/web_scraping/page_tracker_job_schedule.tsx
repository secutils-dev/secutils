import {
  EuiButtonIcon,
  EuiFieldText,
  EuiFlexGroup,
  EuiFlexItem,
  EuiFormLabel,
  EuiSelect,
  EuiSpacer,
  EuiToolTip,
} from '@elastic/eui';
import { useCallback, useEffect, useMemo, useState } from 'react';

import type { AnchorParams, RetryInterval } from './consts';
import {
  buildAnchoredCron,
  DAY_OF_MONTH_OPTIONS,
  defaultAnchorParams,
  detectSchedulePreset,
  getRetryIntervals,
  getScheduleMinInterval,
  HOUR_OPTIONS,
  MINUTE_OPTIONS,
  PAGE_TRACKER_CUSTOM_SCHEDULE,
  PAGE_TRACKER_MANUAL_SCHEDULE,
  PAGE_TRACKER_SCHEDULES,
  parseAnchorParams,
  WEEKDAY_OPTIONS,
} from './consts';
import { type AsyncData, getErrorMessage, ResponseError } from '../../../../model';
import { useWorkspaceContext } from '../../hooks';

export interface PageTrackerJobScheduleProps {
  schedule?: string;
  onChange: (schedule: string | null, retryIntervals: RetryInterval[]) => void;
}

interface ScheduleCheck {
  minInterval: number;
  nextOccurrences: number[];
}

const PRESET_TYPES = new Set(['@hourly', '@daily', '@weekly', '@monthly']);

function isPreset(type: string): boolean {
  return PRESET_TYPES.has(type);
}

function initAnchorParams(schedule: string | undefined, preset: string | null): AnchorParams {
  if (schedule && preset) {
    const parsed = parseAnchorParams(schedule);
    if (parsed) {
      return parsed;
    }
  }
  return defaultAnchorParams();
}

export function PageTrackerJobSchedule({ schedule, onChange }: PageTrackerJobScheduleProps) {
  const { uiState } = useWorkspaceContext();

  const subscriptionSchedules = uiState.subscription?.features?.webScraping.trackerSchedules;
  const schedules = subscriptionSchedules
    ? PAGE_TRACKER_SCHEDULES.filter((knownSchedule) => subscriptionSchedules.includes(knownSchedule.value))
    : PAGE_TRACKER_SCHEDULES;

  const detectedPreset = useMemo(() => (schedule ? detectSchedulePreset(schedule) : null), [schedule]);

  const [scheduleType, setScheduleType] = useState<string>(() => {
    if (!schedule) {
      return PAGE_TRACKER_MANUAL_SCHEDULE;
    }
    if (schedules.some((s) => s.value === schedule)) {
      return schedule;
    }
    if (detectedPreset && schedules.some((s) => s.value === detectedPreset)) {
      return detectedPreset;
    }
    return PAGE_TRACKER_CUSTOM_SCHEDULE;
  });

  const [anchorParams, setAnchorParams] = useState<AnchorParams>(() => initAnchorParams(schedule, detectedPreset));

  const [customSchedule, setCustomSchedule] = useState<string>(
    scheduleType === PAGE_TRACKER_CUSTOM_SCHEDULE ? (schedule ?? '') : '',
  );
  const [customScheduleValidated, setCustomScheduleValidated] = useState<boolean>(false);
  const [scheduleCheck, setScheduleCheck] = useState<AsyncData<ScheduleCheck> | null>(null);

  const anchoredCron = useMemo(
    () => (isPreset(scheduleType) ? buildAnchoredCron(scheduleType, anchorParams) : null),
    [scheduleType, anchorParams],
  );

  const scheduleToCheck =
    scheduleType === PAGE_TRACKER_MANUAL_SCHEDULE
      ? null
      : scheduleType === PAGE_TRACKER_CUSTOM_SCHEDULE
        ? customSchedule || null
        : anchoredCron;

  useEffect(() => {
    if (!scheduleToCheck) {
      setScheduleCheck(null);
      return;
    }

    if (scheduleType === PAGE_TRACKER_CUSTOM_SCHEDULE && customScheduleValidated) {
      return;
    }

    setScheduleCheck({ status: 'pending' });

    fetch('/api/scheduler/parse_schedule', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ schedule: scheduleToCheck }),
    })
      .then(async (res) => {
        if (!res.ok) {
          throw await ResponseError.fromResponse(res);
        }

        const checkResult = (await res.json()) as ScheduleCheck;
        setScheduleCheck({ status: 'succeeded', data: checkResult });

        if (scheduleType === PAGE_TRACKER_CUSTOM_SCHEDULE) {
          setCustomScheduleValidated(true);
          onChange(customSchedule, getRetryIntervals(checkResult.minInterval));
        }
      })
      .catch((e) => {
        setScheduleCheck({ status: 'failed', error: getErrorMessage(e) });
        if (scheduleType === PAGE_TRACKER_CUSTOM_SCHEDULE) {
          setCustomScheduleValidated(true);
          onChange('', []);
        }
      });
  }, [scheduleToCheck, customScheduleValidated, scheduleType, customSchedule, onChange]);

  const onAnchorChange = useCallback(
    (newParams: AnchorParams) => {
      setAnchorParams(newParams);
      const cron = buildAnchoredCron(scheduleType, newParams);
      onChange(cron, getRetryIntervals(getScheduleMinInterval(scheduleType)));
    },
    [scheduleType, onChange],
  );

  const calendarButton =
    scheduleType === PAGE_TRACKER_MANUAL_SCHEDULE ? (
      <EuiButtonIcon iconType={'calendar'} aria-label="Next occurrences aren't available" isDisabled />
    ) : scheduleCheck?.status === 'succeeded' ? (
      <EuiToolTip
        title="Upcoming checks"
        position={'bottom'}
        content={
          <>
            {scheduleCheck.data.nextOccurrences.map((occurrence) => (
              <p key={occurrence}>{new Date(occurrence * 1000).toUTCString()}</p>
            ))}
          </>
        }
      >
        <EuiButtonIcon iconType={'calendar'} aria-label="Show next occurrences" />
      </EuiToolTip>
    ) : (
      <EuiButtonIcon iconType={'calendar'} aria-label="Next occurrences aren't available" isDisabled />
    );

  const typePicker = (
    <EuiSelect
      options={schedules}
      value={scheduleType}
      append={calendarButton}
      onChange={(e) => {
        const newType = e.target.value;
        setScheduleType(newType);
        setCustomSchedule('');
        setCustomScheduleValidated(false);
        setScheduleCheck(null);

        if (newType === PAGE_TRACKER_MANUAL_SCHEDULE || newType === PAGE_TRACKER_CUSTOM_SCHEDULE) {
          onChange(null, []);
        } else if (isPreset(newType)) {
          const params = defaultAnchorParams();
          setAnchorParams(params);
          onChange(buildAnchoredCron(newType, params), getRetryIntervals(getScheduleMinInterval(newType)));
        }
      }}
    />
  );

  const anchorControls = isPreset(scheduleType) ? (
    <>
      <EuiSpacer size={'s'} />
      <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false} wrap>
        {scheduleType === '@weekly' && (
          <>
            <EuiFlexItem grow={false}>
              <EuiFormLabel>on</EuiFormLabel>
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiSelect
                compressed
                options={WEEKDAY_OPTIONS}
                value={String(anchorParams.weekday)}
                onChange={(e) => onAnchorChange({ ...anchorParams, weekday: parseInt(e.target.value, 10) })}
              />
            </EuiFlexItem>
          </>
        )}
        {scheduleType === '@monthly' && (
          <>
            <EuiFlexItem grow={false}>
              <EuiFormLabel>on day</EuiFormLabel>
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiSelect
                compressed
                options={DAY_OF_MONTH_OPTIONS}
                value={String(anchorParams.dayOfMonth)}
                onChange={(e) => onAnchorChange({ ...anchorParams, dayOfMonth: parseInt(e.target.value, 10) })}
              />
            </EuiFlexItem>
          </>
        )}
        <EuiFlexItem grow={false}>
          <EuiFormLabel>{scheduleType === '@hourly' ? 'at minute' : 'at'}</EuiFormLabel>
        </EuiFlexItem>
        {scheduleType !== '@hourly' && (
          <>
            <EuiFlexItem grow={false}>
              <EuiSelect
                compressed
                options={HOUR_OPTIONS}
                value={String(anchorParams.hour)}
                onChange={(e) => onAnchorChange({ ...anchorParams, hour: parseInt(e.target.value, 10) })}
              />
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiFormLabel>:</EuiFormLabel>
            </EuiFlexItem>
          </>
        )}
        <EuiFlexItem grow={false}>
          <EuiSelect
            compressed
            options={MINUTE_OPTIONS}
            value={String(anchorParams.minute)}
            onChange={(e) => onAnchorChange({ ...anchorParams, minute: parseInt(e.target.value, 10) })}
          />
        </EuiFlexItem>
        <EuiFlexItem grow={false}>
          <EuiFormLabel>UTC</EuiFormLabel>
        </EuiFlexItem>
      </EuiFlexGroup>
    </>
  ) : null;

  if (scheduleType !== PAGE_TRACKER_CUSTOM_SCHEDULE) {
    return (
      <div>
        {typePicker}
        {anchorControls}
      </div>
    );
  }

  return (
    <div>
      {typePicker}
      <EuiSpacer size={'s'} />
      <EuiFieldText
        placeholder={'Cron expression, e.g., 0 30 9 * * Mon,Wed,Sat'}
        value={customSchedule}
        isLoading={scheduleCheck?.status === 'pending'}
        isInvalid={scheduleCheck?.status === 'failed'}
        onChange={(e) => {
          setCustomSchedule(e.target.value);
          setCustomScheduleValidated(false);
        }}
      />
    </div>
  );
}
