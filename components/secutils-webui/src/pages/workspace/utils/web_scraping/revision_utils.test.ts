// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';

import { containsHTMLTags, detectLanguage, revisionDataToString } from './revision_utils';

describe('containsHTMLTags', () => {
  it('returns true for HTML with element nodes', () => {
    expect(containsHTMLTags('<div>Hello</div>')).toBe(true);
    expect(containsHTMLTags('<p>paragraph</p>')).toBe(true);
    expect(containsHTMLTags('<html><body><h1>Title</h1></body></html>')).toBe(true);
  });

  it('returns true for self-closing HTML elements', () => {
    expect(containsHTMLTags('<br/>')).toBe(true);
    expect(containsHTMLTags('<img src="test.png"/>')).toBe(true);
  });

  it('returns false for plain text', () => {
    expect(containsHTMLTags('Hello World')).toBe(false);
    expect(containsHTMLTags('no tags here at all')).toBe(false);
  });

  it('returns false for empty string', () => {
    expect(containsHTMLTags('')).toBe(false);
  });

  it('returns false for JSON-like strings', () => {
    expect(containsHTMLTags('{"key": "value"}')).toBe(false);
  });

  it('returns true for mixed content with HTML tags', () => {
    expect(containsHTMLTags('Some text <b>bold</b> more text')).toBe(true);
  });
});

describe('detectLanguage', () => {
  it('detects HTML content', () => {
    expect(detectLanguage('<div>Hello</div>')).toBe('html');
    expect(detectLanguage('<html><body><p>text</p></body></html>')).toBe('html');
  });

  it('detects JSON objects', () => {
    expect(detectLanguage('{"key": "value"}')).toBe('json');
    expect(detectLanguage('[1, 2, 3]')).toBe('json');
  });

  it('returns text for JSON primitives', () => {
    expect(detectLanguage('"just a string"')).toBe('text');
    expect(detectLanguage('42')).toBe('text');
  });

  it('returns text for plain text', () => {
    expect(detectLanguage('Hello World')).toBe('text');
    expect(detectLanguage('no tags here')).toBe('text');
  });

  it('returns text for invalid JSON', () => {
    expect(detectLanguage('{invalid json}')).toBe('text');
  });
});

describe('revisionDataToString', () => {
  it('returns string data as-is', () => {
    expect(revisionDataToString('hello')).toBe('hello');
    expect(revisionDataToString('<div>html</div>')).toBe('<div>html</div>');
  });

  it('pretty-prints objects', () => {
    const result = revisionDataToString({ key: 'value' });
    expect(result).toBe('{\n  "key": "value"\n}');
  });

  it('pretty-prints arrays', () => {
    const result = revisionDataToString([1, 2, 3]);
    expect(result).toBe('[\n  1,\n  2,\n  3\n]');
  });

  it('handles null', () => {
    expect(revisionDataToString(null)).toBe('null');
  });

  it('handles numbers', () => {
    expect(revisionDataToString(42)).toBe('42');
  });
});
