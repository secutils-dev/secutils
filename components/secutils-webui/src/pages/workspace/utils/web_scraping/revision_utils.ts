export function containsHTMLTags(data: string): boolean {
  if (!data) {
    return false;
  }

  try {
    const doc = new DOMParser().parseFromString(data, 'text/html');
    return Array.from(doc.body.childNodes).some((node) => node.nodeType === Node.ELEMENT_NODE);
  } catch {
    return false;
  }
}

export function detectLanguage(data: string): 'html' | 'json' | 'text' {
  if (containsHTMLTags(data)) {
    return 'html';
  }

  try {
    const parsed = JSON.parse(data);
    if (typeof parsed === 'object' && parsed !== null) {
      return 'json';
    }
  } catch {
    // Not JSON
  }
  return 'text';
}

export function revisionDataToString(data: unknown): string {
  return typeof data === 'string' ? data : JSON.stringify(data, null, 2);
}
