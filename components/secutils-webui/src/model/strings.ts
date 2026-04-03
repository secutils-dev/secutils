const COPY_NAME_REGEX = /^(.*?)(?: \(Copy (\d+)\))?$/;

export function getCopyName(name: string, existingNames?: Iterable<string>): string {
  const match = name.match(COPY_NAME_REGEX);
  const base = match ? match[1] : name;
  let copyNum = match?.[2] ? parseInt(match[2], 10) + 1 : 1;

  if (existingNames) {
    const taken = new Set(existingNames);
    while (taken.has(`${base} (Copy ${copyNum})`)) {
      copyNum++;
    }
  }

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
