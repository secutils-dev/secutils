import type { Dispatch, SetStateAction } from 'react';
import { useEffect, useState } from 'react';

export function useLocalStorage<TValue>(key: string, defaultValue: TValue) {
  const [storedValue, setStoredValue] = useState<TValue>(() => {
    try {
      const item = window.localStorage.getItem(key);
      return item ? (JSON.parse(item) as TValue) : defaultValue;
    } catch (err) {
      console.error(err);
      return defaultValue;
    }
  });

  useEffect(() => {
    try {
      if (storedValue != null) {
        window.localStorage.setItem(key, JSON.stringify(storedValue));
      } else {
        window.localStorage.removeItem(key);
      }
    } catch (err) {
      console.error(err);
    }
  }, [storedValue]);

  return [storedValue, setStoredValue] as [TValue, Dispatch<SetStateAction<TValue>>];
}
