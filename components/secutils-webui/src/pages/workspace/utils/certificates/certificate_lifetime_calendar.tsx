import { EuiDatePicker } from '@elastic/eui';
import type { Moment } from 'moment';
import { unix } from 'moment';
import { useCallback, useState } from 'react';

export interface CertificateLifetimeCalendarProps {
  isDisabled?: boolean;
  currentTimestamp: number;
  onChange(timestamp: number): void;
}

export function CertificateLifetimeCalendar({
  onChange,
  currentTimestamp,
  isDisabled = false,
}: CertificateLifetimeCalendarProps) {
  const [selectedDate, setSelectedDate] = useState<Moment | null>(unix(currentTimestamp));
  const onSelectedDateChange = useCallback(
    (selectedDate: Moment | null) => {
      setSelectedDate(selectedDate);

      if (selectedDate) {
        onChange(selectedDate.unix());
      }
    },
    [onChange],
  );

  return (
    <EuiDatePicker
      selected={selectedDate}
      disabled={isDisabled}
      dateFormat={'LL HH:mm'}
      showTimeSelect
      onChange={onSelectedDateChange}
    />
  );
}
