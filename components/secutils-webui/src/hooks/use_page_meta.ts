import { useEffect } from 'react';

export function usePageMeta(pageTitle: string) {
  useEffect(() => {
    document.title = `Secutils.dev - ${pageTitle}`;
  }, [pageTitle]);
}
