/*
 * Glyphs from Elastic EUI 119, Copyright Elasticsearch B.V.
 * Licensed under the Elastic License 2.0 or Server Side Public License, v1.
 */
import type { SVGProps } from 'react';

export function ScriptIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg" {...props}>
      <path d="M13 2.226v2.218c-.359-.143-.845-.218-1.315-.218-1.059 0-1.631.519-1.802 1.565l-.168.937h2.798v2.159H9.41l-.313 1.674C8.696 12.987 7.261 14 4.785 14c-.718 0-1.35-.092-1.785-.251v-2.243c.418.176.905.268 1.383.268 1.008 0 1.546-.435 1.725-1.523l.24-1.364H3.787V6.728h2.812l.288-1.264C7.286 3.071 8.662 2 11.352 2c.598 0 1.306.1 1.648.226z" />
    </svg>
  );
}

export function EmptyHistoryIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg" {...props}>
      <path
        fillRule="evenodd"
        d="M13.657 3.05a.5.5 0 0 0-.707-.707l-.366.366A7 7 0 1 0 8 15a4.994 4.994 0 0 1-.597-1.03 6 6 0 1 1 4.471-10.552l-.71.71a5 5 0 1 0-4.08 8.788 5.027 5.027 0 0 1-.082-1.042A4.002 4.002 0 0 1 8 4a3.98 3.98 0 0 1 2.453.84l-.715.714a3 3 0 0 0-3.86 4.567.5.5 0 1 0 .708-.707 2 2 0 0 1 2.43-3.137l-.757.757a1 1 0 1 0 .707.707l1.155-1.155 2.46-2.46a5.972 5.972 0 0 1 1.39 3.277c.367.158.713.36 1.029.597 0-1.636-.57-3.271-1.71-4.584l.367-.366zM16 12a4 4 0 1 1-8 0 4 4 0 0 1 8 0zm-4 .5a.577.577 0 0 1-.57-.495l-.29-2.015a.867.867 0 1 1 1.718 0l-.288 2.015a.577.577 0 0 1-.57.495zm0 2.5a1 1 0 1 0 0-2 1 1 0 0 0 0 2z"
      />
    </svg>
  );
}
