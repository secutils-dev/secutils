import type { EuiDataGridProps } from '@elastic/eui';
import { EuiDataGrid } from '@elastic/eui';
import { useEffect, useRef, useState } from 'react';

/**
 * Wrapper around EuiDataGrid that fixes the fullscreen restore bug by forcing
 * a re-render when exiting fullscreen mode.
 */
export function DataGrid(props: EuiDataGridProps) {
  const [gridKey, setGridKey] = useState(0);
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const grid = gridRef.current?.querySelector('.euiDataGrid');
    if (!grid) {
      return;
    }

    let wasFullscreen = grid.classList.contains('euiDataGrid--fullScreen');
    const observer = new MutationObserver(() => {
      const isFullscreen = grid.classList.contains('euiDataGrid--fullScreen');
      if (wasFullscreen && !isFullscreen) {
        setGridKey((k) => k + 1);
      }
      wasFullscreen = isFullscreen;
    });

    observer.observe(grid, { attributes: true, attributeFilter: ['class'] });
    return () => observer.disconnect();
  }, [gridKey]);

  return (
    <div ref={gridRef}>
      <EuiDataGrid key={gridKey} {...props} />
    </div>
  );
}
