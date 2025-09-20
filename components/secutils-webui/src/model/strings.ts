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
