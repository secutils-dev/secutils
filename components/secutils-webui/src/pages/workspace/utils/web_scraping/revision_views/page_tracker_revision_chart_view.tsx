import { EuiButtonIcon, EuiFlexGroup, EuiFlexItem, EuiFocusTrap, EuiPanel, useEuiTheme } from '@elastic/eui';
import { css } from '@emotion/react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { createPortal } from 'react-dom';
import { Area, AreaChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts';

import type { ChartDataPoint } from './page_tracker_revision_chart_utils';
import { formatChartValue, revisionsToChartData } from './page_tracker_revision_chart_utils';
import type { TrackerDataRevision } from '../tracker_data_revision';

export interface PageTrackerRevisionChartViewProps {
  revisions: TrackerDataRevision[];
}

function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

function formatTooltipTimestamp(timestamp: number): string {
  return new Date(timestamp).toLocaleString();
}

interface CustomTooltipProps {
  active?: boolean;
  payload?: Array<{ payload: ChartDataPoint }>;
  primaryColor: string;
}

function CustomTooltip({ active, payload, primaryColor }: CustomTooltipProps) {
  if (!active || !payload || payload.length === 0) {
    return null;
  }

  const data = payload[0].payload;
  return (
    <div
      style={{
        backgroundColor: 'var(--eui-background-color-plain)',
        border: '1px solid var(--eui-border-color-primary)',
        borderRadius: '4px',
        padding: '8px 12px',
        boxShadow: '0 2px 8px rgba(0, 0, 0, 0.15)',
      }}
    >
      <div style={{ fontSize: '12px', color: 'var(--eui-text-color-subdued)', marginBottom: '4px' }}>
        {formatTooltipTimestamp(data.timestamp)}
      </div>
      <div style={{ fontSize: '14px', fontWeight: 600, color: primaryColor }}>{formatChartValue(data.value)}</div>
    </div>
  );
}

interface ChartContentProps {
  chartData: ChartDataPoint[];
  yDomain: [number, number];
  height: `${number}%` | number;
  isFullScreen?: boolean;
  onToggleFullScreen?: () => void;
}

function ChartContent({ chartData, yDomain, height, isFullScreen, onToggleFullScreen }: ChartContentProps) {
  const { euiTheme } = useEuiTheme();

  const primaryColor = euiTheme.colors.primary;
  const gridColor = euiTheme.colors.lightShade;
  const textColor = euiTheme.colors.subduedText;

  // Use unique gradient ID to avoid conflicts between normal and fullscreen views
  const gradientId = isFullScreen ? 'colorValueFullScreen' : 'colorValue';

  return (
    <EuiFlexGroup direction="column" gutterSize="none" style={{ height: '100%' }}>
      <EuiFlexItem grow={false}>
        <EuiFlexGroup justifyContent="flexEnd" gutterSize="none">
          <EuiFlexItem grow={false}>
            <EuiButtonIcon
              iconType={isFullScreen ? 'fullScreenExit' : 'fullScreen'}
              aria-label={isFullScreen ? 'Exit full screen' : 'Enter full screen'}
              onClick={onToggleFullScreen}
              color="text"
            />
          </EuiFlexItem>
        </EuiFlexGroup>
      </EuiFlexItem>
      <EuiFlexItem>
        <ResponsiveContainer width="100%" height={height}>
          <AreaChart data={chartData} margin={{ top: 10, right: 30, left: 10, bottom: 10 }}>
            <defs>
              <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor={primaryColor} stopOpacity={0.3} />
                <stop offset="95%" stopColor={primaryColor} stopOpacity={0} />
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke={gridColor} vertical={false} />
            <XAxis
              dataKey="timestamp"
              tickFormatter={formatTimestamp}
              stroke={textColor}
              fontSize={12}
              tickLine={false}
              axisLine={{ stroke: gridColor }}
            />
            <YAxis
              domain={yDomain}
              tickFormatter={formatChartValue}
              stroke={textColor}
              fontSize={12}
              tickLine={false}
              axisLine={false}
              width={80}
            />
            <Tooltip content={<CustomTooltip primaryColor={primaryColor} />} />
            <Area
              type="monotone"
              dataKey="value"
              stroke={primaryColor}
              strokeWidth={2}
              fillOpacity={1}
              fill={`url(#${gradientId})`}
              dot={{ fill: primaryColor, strokeWidth: 2, r: 4 }}
              activeDot={{ r: 6, stroke: primaryColor, strokeWidth: 2, fill: 'var(--eui-background-color-plain)' }}
            />
          </AreaChart>
        </ResponsiveContainer>
      </EuiFlexItem>
    </EuiFlexGroup>
  );
}

interface FullScreenChartProps {
  chartData: ChartDataPoint[];
  yDomain: [number, number];
  onClose: () => void;
}

function FullScreenChart({ chartData, yDomain, onClose }: FullScreenChartProps) {
  const { euiTheme } = useEuiTheme();

  // Handle escape key to close fullscreen
  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        onClose();
      }
    },
    [onClose],
  );

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);
    // Prevent body scroll when the fullscreen is open.
    document.body.style.overflow = 'hidden';

    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.body.style.overflow = '';
    };
  }, [handleKeyDown]);

  return createPortal(
    <EuiFocusTrap onClickOutside={onClose}>
      <div
        css={css`
          animation: euiFullScreenOverlay 350ms cubic-bezier(0.34, 1.56, 0.64, 1);
          position: fixed;
          inset: 0;
          z-index: ${euiTheme.levels.modal};
          display: flex;
          flex-direction: column;
          background-color: ${euiTheme.colors.body};

          @keyframes euiFullScreenOverlay {
            0% {
              opacity: 0;
              transform: translateY(16px);
            }
            100% {
              opacity: 1;
              transform: translateY(0);
            }
          }
        `}
      >
        <EuiPanel
          paddingSize="l"
          css={css`
            height: 100%;
            display: flex;
            flex-direction: column;
          `}
        >
          <ChartContent
            chartData={chartData}
            yDomain={yDomain}
            height="100%"
            isFullScreen={true}
            onToggleFullScreen={onClose}
          />
        </EuiPanel>
      </div>
    </EuiFocusTrap>,
    document.body,
  );
}

export function PageTrackerRevisionChartView({ revisions }: PageTrackerRevisionChartViewProps) {
  const [isFullScreen, setIsFullScreen] = useState(false);

  const chartData = useMemo(() => revisionsToChartData(revisions), [revisions]);

  // Calculate Y-axis domain with some padding
  const values = chartData.map((d) => d.value);
  const minValue = Math.min(...values);
  const maxValue = Math.max(...values);
  const range = maxValue - minValue;
  const padding = range * 0.1 || Math.abs(minValue) * 0.1 || 1;
  const yDomain: [number, number] = [minValue - padding, maxValue + padding];

  const toggleFullScreen = useCallback(() => setIsFullScreen((prev) => !prev), []);

  return (
    <>
      <ChartContent
        chartData={chartData}
        yDomain={yDomain}
        height={300}
        isFullScreen={false}
        onToggleFullScreen={toggleFullScreen}
      />
      {isFullScreen && <FullScreenChart chartData={chartData} yDomain={yDomain} onClose={toggleFullScreen} />}
    </>
  );
}
