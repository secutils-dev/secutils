import { useEuiFontSize, useIsWithinMaxBreakpoint } from '@elastic/eui';

export function useFontSizes() {
  const isWithinMaxBreakpoint = useIsWithinMaxBreakpoint('l');

  return {
    text: isWithinMaxBreakpoint ? useEuiFontSize('m') : useEuiFontSize('l'),
    codeSample: isWithinMaxBreakpoint ? ('m' as const) : ('l' as const),
  };
}
