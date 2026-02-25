import { useState } from 'react';

/**
 * Returns true if the current form values differ from those captured on mount.
 * Pass a JSON-serializable snapshot of the form's current state — the first
 * render's snapshot becomes the baseline for all future comparisons.
 */
export function useFormChanges(currentValues: unknown): boolean {
  const serialized = JSON.stringify(currentValues);
  const [initialSnapshot] = useState(serialized);
  return serialized !== initialSnapshot;
}
