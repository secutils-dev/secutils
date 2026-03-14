const COPY_NAME_REGEX = /^(.*?)(?: \(Copy (\d+)\))?$/;

export function getCopyName(name: string): string {
  const match = name.match(COPY_NAME_REGEX);
  if (!match) {
    return `${name} (Copy 1)`;
  }

  const base = match[1];
  const copyNum = match[2] ? parseInt(match[2], 10) + 1 : 1;
  return `${base} (Copy ${copyNum})`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
