import type { EuiDataGridProps } from '@elastic/eui';
import { EuiDataGrid } from '@elastic/eui';
import { useEffect, useRef, useState } from 'react';

/**
 * Wrapper around EuiDataGrid that fixes the fullscreen restore bug and sidebar
 * expansion layout issues by forcing a re-render.
 */
export function DataGrid(props: EuiDataGridProps) {
  const [gridKey, setGridKey] = useState(0);
  const gridRef = useRef<HTMLDivElement>(null);
  const [isFullScreen, setIsFullScreen] = useState(false);

  useEffect(() => {
    const container = document.querySelector('.root');
    if (!container) {
      return;
    }

    // Observer for container resize (e.g., sidebar expansion).
    let debounceTimerId = 0;
    let width = container.clientWidth;
    const resizeObserver = new ResizeObserver(() => {
      clearTimeout(debounceTimerId);

      const newWidth = container.clientWidth;
      if (newWidth < width) {
        debounceTimerId = setTimeout(() => setGridKey((k) => k + 1), 100);
      }
      width = newWidth;
    });
    resizeObserver.observe(container);

    return () => {
      clearTimeout(debounceTimerId);
      resizeObserver.disconnect();
    };
  }, []);

  return (
    <div ref={gridRef}>
      <EuiDataGrid
        key={gridKey}
        onFullScreenChange={(newIsFullScreen) => {
          if (isFullScreen && !newIsFullScreen) {
            setGridKey((k) => k + 1);
          }
          setIsFullScreen(newIsFullScreen);
        }}
        {...props}
      />
    </div>
  );
}
