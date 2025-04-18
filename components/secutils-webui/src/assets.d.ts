declare module '*.png' {
  const path: string;
  export default path;
}

declare module '*.webm' {
  const path: string;
  export default path;
}

declare module '*.mp4' {
  const path: string;
  export default path;
}

declare module '@elastic/eui/es/components/icon/icon' {
  import type { SVGProps } from 'react';
  export function appendIconComponentCache(icons: Record<string, SVGProps<SVGSVGElement>>): void;
}

declare module '@elastic/eui/es/components/icon/assets/*' {
  import type { SVGProps } from 'react';
  export const icon: SVGProps<SVGSVGElement>;
}

declare module '@elastic/eui/dist/*.min.css' {
  const path: string;
  export default path;
}

declare module 'url:monaco-editor/*' {
  const path: string;
  export default path;
}
