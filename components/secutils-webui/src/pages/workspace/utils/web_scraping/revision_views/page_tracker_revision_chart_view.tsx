import { useEuiTheme } from '@elastic/eui';
import { useMemo } from 'react';
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

export function PageTrackerRevisionChartView({ revisions }: PageTrackerRevisionChartViewProps) {
  const { euiTheme } = useEuiTheme();

  const chartData = useMemo(() => revisionsToChartData(revisions), [revisions]);

  const primaryColor = euiTheme.colors.primary;
  const gridColor = euiTheme.colors.lightShade;
  const textColor = euiTheme.colors.subduedText;

  // Calculate Y-axis domain with some padding
  const values = chartData.map((d) => d.value);
  const minValue = Math.min(...values);
  const maxValue = Math.max(...values);
  const range = maxValue - minValue;
  const padding = range * 0.1 || Math.abs(minValue) * 0.1 || 1;
  const yDomain: [number, number] = [minValue - padding, maxValue + padding];

  return (
    <ResponsiveContainer width="100%" height={300}>
      <AreaChart data={chartData} margin={{ top: 10, right: 30, left: 10, bottom: 10 }}>
        <defs>
          <linearGradient id="colorValue" x1="0" y1="0" x2="0" y2="1">
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
          fill="url(#colorValue)"
          dot={{ fill: primaryColor, strokeWidth: 2, r: 4 }}
          activeDot={{ r: 6, stroke: primaryColor, strokeWidth: 2, fill: 'var(--eui-background-color-plain)' }}
        />
      </AreaChart>
    </ResponsiveContainer>
  );
}
