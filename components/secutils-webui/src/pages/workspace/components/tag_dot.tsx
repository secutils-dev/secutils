import type { EuiThemeComputed } from '@elastic/eui';
import { useEuiTheme } from '@elastic/eui';

const NAMED_TAG_COLORS: Record<string, (theme: EuiThemeComputed) => string> = {
  default: (theme) => theme.colors.textSubdued,
  primary: (theme) => theme.colors.primary,
  success: (theme) => theme.colors.success,
  accent: (theme) => theme.colors.accent,
  warning: (theme) => theme.colors.warning,
  danger: (theme) => theme.colors.danger,
};

export function TagDot({ color, size = 8 }: { color: string; size?: number }) {
  const { euiTheme } = useEuiTheme();
  // The zero-width space gives the dot a text line even inside flex containers (like `EuiSelectable` prepends),
  // so `vertical-align: middle` centers it on the lowercase letters rather than on the taller line box.
  return (
    <span aria-hidden={true} style={{ flexShrink: 0 }}>
      {'\u200B'}
      <span
        style={{
          display: 'inline-block',
          verticalAlign: 'middle',
          width: size,
          height: size,
          borderRadius: '50%',
          backgroundColor: NAMED_TAG_COLORS[color]?.(euiTheme) ?? color,
          // Inset, so the ring that outlines very light colors is never clipped by overflow-hidden parents.
          boxShadow: 'inset 0 0 0 1px rgba(0, 0, 0, 0.15)',
        }}
      />
    </span>
  );
}
