import { useIsWithinMaxBreakpoint } from '@elastic/eui';

export function useRangeTicks() {
  const isWithinMaxBreakpoint = useIsWithinMaxBreakpoint('xs');
  // Determines the maximum value for the range to show ticks.
  return isWithinMaxBreakpoint ? 10 : 15;
}
