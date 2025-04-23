import { useEuiFontSize, useIsWithinMaxBreakpoint } from '@elastic/eui';

export function useFontSizes() {
  const isWithinMaxBreakpoint = useIsWithinMaxBreakpoint('l');

  return {
    text: useEuiFontSize(isWithinMaxBreakpoint ? 'm' : 'l'),
    codeSample: isWithinMaxBreakpoint ? ('m' as const) : ('l' as const),
  };
}
