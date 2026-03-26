import React from 'react';
import CodeBlock from '@theme/CodeBlock';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';

import './SampleFields.scss';

interface FieldMapping {
  /** Label shown in the left column (e.g., "Name", "Path"). */
  label: string;
  /**
   * Dot-separated path into the sample entity to extract the value.
   * Examples: "name", "location.path", "settings.body", "settings.headers".
   */
  path: string;
  /** Optional language hint for the code block (e.g., "html", "json", "javascript", "http"). */
  language?: string;
}

interface StaticField {
  /** Label shown in the left column. */
  label: string;
  /** Literal value to display. */
  value: string;
  /** Optional language hint for the code block. */
  language?: string;
}

interface SampleFieldsProps {
  /** The imported sample JSON object (use `require('@site/static/samples/...')` in MDX). */
  sample: {
    version: number;
    data: Record<string, unknown[]>;
  };
  /** Entity category key in the data object (e.g., "responders", "contentSecurityPolicies"). */
  entity: string;
  /** Index of the entity in the array (defaults to 0). */
  index?: number;
  /** Field mappings describing which fields to show. */
  fields: FieldMapping[];
  /** Relative path from static/samples/ used to construct the import URL (without .secutils.json extension). */
  samplePath: string;
  /** When true, only the fields table is rendered without the "Import this sample" button. */
  hideImportButton?: boolean;
  /** Static rows appended after the sample-derived rows (e.g., fields not present in the sample file). */
  extraFields?: StaticField[];
}

/**
 * Resolve a dot-separated path on an object.
 * Special handling for `settings.headers` which is stored as `[["key", "value"], ...]`
 * and rendered as HTTP-style `key: value` lines.
 */
function resolveValue(obj: unknown, path: string): string | undefined {
  const parts = path.split('.');
  let current: unknown = obj;
  for (const part of parts) {
    if (current == null || typeof current !== 'object') return undefined;
    if (Array.isArray(current)) {
      const idx = Number(part);
      if (Number.isNaN(idx)) return undefined;
      current = current[idx];
    } else {
      current = (current as Record<string, unknown>)[part];
    }
  }

  if (current == null) return undefined;

  // Headers are stored as [["Content-Type", "text/html; charset=utf-8"], ...]
  if (path === 'settings.headers' && Array.isArray(current)) {
    return current.map(([k, v]: [string, string]) => `${k}: ${v}`).join('\n');
  }

  // Directives for CSP are stored as [{name, value}, ...] - render as "directive source1 source2"
  if (path === 'directives' && Array.isArray(current)) {
    return current.map((d: { name: string; value: string[] }) => `${d.name} ${d.value.join(' ')}`).join('\n');
  }

  // Certificate isCa boolean → human-readable label
  if (path === 'attributes.isCa' && typeof current === 'boolean') {
    return current ? 'Certificate Authority' : 'End Entity';
  }

  // keyUsage / extendedKeyUsage arrays - convert camelCase to readable labels
  if ((path === 'attributes.keyUsage' || path === 'attributes.extendedKeyUsage') && Array.isArray(current)) {
    return current.map((v: string) => v.replace(/([A-Z])/g, ' $1').replace(/^./, (c) => c.toUpperCase()).trim()).join(', ');
  }

  // Private key encrypted boolean → human-readable label
  if (path === 'encrypted' && typeof current === 'boolean') {
    return current ? 'Passphrase' : 'None';
  }

  if (typeof current === 'object') {
    return JSON.stringify(current, null, 2);
  }

  return String(current);
}

export default function SampleFields({
  sample,
  entity,
  index = 0,
  fields,
  samplePath,
  hideImportButton = false,
  extraFields,
}: SampleFieldsProps): React.ReactElement | null {
  const { siteConfig } = useDocusaurusContext();
  const baseUrl = siteConfig.customFields?.baseUrl as string;

  const entities = sample?.data?.[entity];
  if (!Array.isArray(entities) || entities.length <= index) {
    return <p>Sample data not found for {entity}[{index}].</p>;
  }
  const entityData = entities[index];

  const importUrl = `${baseUrl}/ws/settings?import_url=${baseUrl}/docs/samples/${samplePath}.secutils.json`;

  return (
    <div className="su-sample-fields">
      <table className="su-table">
        <tbody>
          {fields.map(({ label, path, language }) => {
            const value = resolveValue(entityData, path);
            if (value === undefined) return null;
            return (
              <tr key={path}>
                <td>
                  <b>{label}</b>
                </td>
                <td>
                  <CodeBlock language={language}>{value}</CodeBlock>
                </td>
              </tr>
            );
          })}
          {extraFields?.map(({ label, value, language }) => (
            <tr key={label}>
              <td>
                <b>{label}</b>
              </td>
              <td>
                <CodeBlock language={language}>{value}</CodeBlock>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {!hideImportButton && (
        <a className="su-sample-fields__import-btn" href={importUrl} target="_blank" rel="noopener noreferrer">
          Import this sample
        </a>
      )}
    </div>
  );
}
