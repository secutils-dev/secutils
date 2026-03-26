import type { EuiBasicTableColumn } from '@elastic/eui';
import {
  EuiBasicTable,
  EuiButton,
  EuiButtonEmpty,
  EuiButtonIcon,
  EuiCallOut,
  EuiCheckbox,
  EuiFieldPassword,
  EuiFlexGroup,
  EuiFlexItem,
  EuiIcon,
  EuiInMemoryTable,
  EuiLoadingSpinner,
  EuiModal,
  EuiModalBody,
  EuiModalFooter,
  EuiModalHeader,
  EuiModalHeaderTitle,
  EuiSpacer,
  EuiSwitch,
  EuiText,
} from '@elastic/eui';
import type { ReactNode } from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { getUserScripts, getUserSecrets } from '../model';
import type { ExportParams } from '../model/user_data_export';
import {
  exportUserData,
  getApiTrackers,
  getCertificateTemplates,
  getContentSecurityPolicies,
  getPageTrackers,
  getPrivateKeys,
  getResponders,
  getTags,
} from '../model/user_data_export';
import type { PageToast } from '../pages/page';
import { Downloader } from '../tools/downloader';

interface Props {
  addToast: (toast: PageToast) => void;
  onClose: () => void;
}

interface SelectionState {
  tags: Set<string>;
  scripts: Set<string>;
  secrets: Set<string>;
  responders: Set<string>;
  certificateTemplates: Set<string>;
  privateKeys: Set<string>;
  contentSecurityPolicies: Set<string>;
  pageTrackers: Set<string>;
  apiTrackers: Set<string>;
}

interface HistoryState {
  responders: boolean;
  pageTrackers: boolean;
  apiTrackers: boolean;
}

type EntityCategory = keyof SelectionState;

interface EntityRow {
  id: EntityCategory | 'settings';
  label: string;
  icon: string;
  historyKey?: keyof HistoryState;
}

const ENTITY_ROWS: EntityRow[] = [
  { id: 'settings', label: 'Settings', icon: 'gear' },
  { id: 'tags', label: 'Tags', icon: 'tag' },
  { id: 'scripts', label: 'Scripts', icon: 'console' },
  { id: 'secrets', label: 'Secrets', icon: 'lock' },
  { id: 'responders', label: 'Responders', icon: 'node', historyKey: 'responders' },
  { id: 'certificateTemplates', label: 'Certificate Templates', icon: 'securityApp' },
  { id: 'privateKeys', label: 'Private Keys', icon: 'securityApp' },
  { id: 'contentSecurityPolicies', label: 'Content Security Policies', icon: 'globe' },
  { id: 'pageTrackers', label: 'Page Trackers', icon: 'cut', historyKey: 'pageTrackers' },
  { id: 'apiTrackers', label: 'API Trackers', icon: 'cut', historyKey: 'apiTrackers' },
];

interface NamedItem {
  id: string;
  name: string;
}

export default function ExportDataModal({ addToast, onClose }: Props) {
  const [loading, setLoading] = useState(true);
  const [exporting, setExporting] = useState(false);
  const [allItems, setAllItems] = useState<Record<EntityCategory, NamedItem[]>>({
    tags: [],
    scripts: [],
    secrets: [],
    responders: [],
    certificateTemplates: [],
    privateKeys: [],
    contentSecurityPolicies: [],
    pageTrackers: [],
    apiTrackers: [],
  });
  const [selection, setSelection] = useState<SelectionState>({
    tags: new Set(),
    scripts: new Set(),
    secrets: new Set(),
    responders: new Set(),
    certificateTemplates: new Set(),
    privateKeys: new Set(),
    contentSecurityPolicies: new Set(),
    pageTrackers: new Set(),
    apiTrackers: new Set(),
  });
  const [history, setHistory] = useState<HistoryState>({
    responders: false,
    pageTrackers: false,
    apiTrackers: false,
  });
  const [expandedRows, setExpandedRows] = useState<Record<string, ReactNode>>({});
  const [includeSettings, setIncludeSettings] = useState(true);
  const [includeSecretValues, setIncludeSecretValues] = useState(false);
  const [secretsPassphrase, setSecretsPassphrase] = useState('');
  const [secretsPassphraseRepeat, setSecretsPassphraseRepeat] = useState('');

  useEffect(() => {
    Promise.all([
      getTags(),
      getUserScripts(),
      getUserSecrets(),
      getResponders(),
      getCertificateTemplates(),
      getPrivateKeys(),
      getContentSecurityPolicies(),
      getPageTrackers(),
      getApiTrackers(),
    ])
      .then(([tags, s, sec, resp, ct, pk, csp, pt, at]) => {
        const items: Record<EntityCategory, NamedItem[]> = {
          tags,
          scripts: s,
          secrets: sec,
          responders: resp,
          certificateTemplates: ct,
          privateKeys: pk,
          contentSecurityPolicies: csp,
          pageTrackers: pt,
          apiTrackers: at,
        };
        setAllItems(items);
        // Select all by default.
        setSelection({
          tags: new Set(tags.map((i) => i.id)),
          scripts: new Set(s.map((i) => i.id)),
          secrets: new Set(sec.map((i) => i.id)),
          responders: new Set(resp.map((i) => i.id)),
          certificateTemplates: new Set(ct.map((i) => i.id)),
          privateKeys: new Set(pk.map((i) => i.id)),
          contentSecurityPolicies: new Set(csp.map((i) => i.id)),
          pageTrackers: new Set(pt.map((i) => i.id)),
          apiTrackers: new Set(at.map((i) => i.id)),
        });
      })
      .catch(() => {
        addToast({ id: 'export-fetch-error', color: 'danger', title: 'Failed to load data for export.' });
      })
      .finally(() => setLoading(false));
  }, [addToast]);

  const toggleItem = useCallback((category: EntityCategory, id: string) => {
    setSelection((prev) => {
      const next = new Set(prev[category]);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return { ...prev, [category]: next };
    });
  }, []);

  const toggleAllInCategory = useCallback(
    (category: EntityCategory) => {
      setSelection((prev) => {
        const items = allItems[category];
        if (prev[category].size === items.length) {
          return { ...prev, [category]: new Set<string>() };
        }
        return { ...prev, [category]: new Set(items.map((i) => i.id)) };
      });
    },
    [allItems],
  );

  const totalSelected = Object.values(selection).reduce((sum, set) => sum + set.size, 0);
  const totalAvailable = Object.values(allItems).reduce((sum, items) => sum + items.length, 0);
  const hasAnythingSelected = totalSelected > 0 || includeSettings;

  const allGloballySelected = totalSelected === totalAvailable && includeSettings;
  const noneGloballySelected = totalSelected === 0 && !includeSettings;

  const toggleAllGlobal = useCallback(() => {
    const shouldSelectAll = !allGloballySelected;
    setIncludeSettings(shouldSelectAll);
    setSelection(() => {
      if (!shouldSelectAll) {
        return {
          tags: new Set(),
          scripts: new Set(),
          secrets: new Set(),
          responders: new Set(),
          certificateTemplates: new Set(),
          privateKeys: new Set(),
          contentSecurityPolicies: new Set(),
          pageTrackers: new Set(),
          apiTrackers: new Set(),
        };
      }
      return {
        tags: new Set(allItems.tags.map((i) => i.id)),
        scripts: new Set(allItems.scripts.map((i) => i.id)),
        secrets: new Set(allItems.secrets.map((i) => i.id)),
        responders: new Set(allItems.responders.map((i) => i.id)),
        certificateTemplates: new Set(allItems.certificateTemplates.map((i) => i.id)),
        privateKeys: new Set(allItems.privateKeys.map((i) => i.id)),
        contentSecurityPolicies: new Set(allItems.contentSecurityPolicies.map((i) => i.id)),
        pageTrackers: new Set(allItems.pageTrackers.map((i) => i.id)),
        apiTrackers: new Set(allItems.apiTrackers.map((i) => i.id)),
      };
    });
  }, [allItems, allGloballySelected]);

  const handleExport = useCallback(async () => {
    setExporting(true);
    try {
      const sel = (key: EntityCategory) => {
        const ids = Array.from(selection[key]);
        return ids.length > 0 ? { type: 'selected' as const, ids } : undefined;
      };
      const trackableSel = (key: EntityCategory, histKey: keyof HistoryState) => {
        const ids = Array.from(selection[key]);
        return ids.length > 0 ? { type: 'selected' as const, ids, includeHistory: history[histKey] } : undefined;
      };
      const params: ExportParams = {
        include: {
          settings: includeSettings || undefined,
          tags: sel('tags'),
          scripts: sel('scripts'),
          secrets: sel('secrets'),
          responders: trackableSel('responders', 'responders'),
          certificateTemplates: sel('certificateTemplates'),
          privateKeys: sel('privateKeys'),
          contentSecurityPolicies: sel('contentSecurityPolicies'),
          pageTrackers: trackableSel('pageTrackers', 'pageTrackers'),
          apiTrackers: trackableSel('apiTrackers', 'apiTrackers'),
        },
        secretsPassphrase: includeSecretValues ? secretsPassphrase : undefined,
      };
      Downloader.download(
        `export-${Date.now()}.secutils.json`,
        new Uint8Array(await (await exportUserData(params)).arrayBuffer()),
        'application/json',
      );
      addToast({ id: 'export-success', color: 'success', title: 'Data exported successfully.' });
      onClose();
    } catch (err) {
      addToast({
        id: 'export-error',
        color: 'danger',
        title: err instanceof Error ? err.message : 'Failed to export data.',
      });
    } finally {
      setExporting(false);
    }
  }, [selection, history, includeSettings, includeSecretValues, secretsPassphrase, addToast, onClose]);

  // Only show entity types that have items (settings row is always visible).
  const visibleRows = useMemo(
    () => ENTITY_ROWS.filter((r) => r.id === 'settings' || allItems[r.id as EntityCategory].length > 0),
    [allItems],
  );

  // Build expanded row content. We need to rebuild when selection changes.
  const buildExpandedContent = useCallback(
    (row: EntityRow): ReactNode => {
      const category = row.id as EntityCategory;
      const items = allItems[category];
      const sel = selection[category];
      const allSelected = sel.size === items.length && items.length > 0;
      const someSelected = sel.size > 0 && sel.size < items.length;

      const innerColumns: Array<EuiBasicTableColumn<NamedItem>> = [
        {
          field: 'name',
          name: (
            <EuiCheckbox
              id={`export-inner-selectall-${row.id}`}
              label="Name"
              checked={allSelected}
              indeterminate={someSelected}
              onChange={() => toggleAllInCategory(category)}
            />
          ),
          render: (_name: string, item: NamedItem) => (
            <EuiCheckbox
              id={`export-inner-${row.id}-${item.id}`}
              label={item.name}
              checked={sel.has(item.id)}
              onChange={() => toggleItem(category, item.id)}
            />
          ),
        },
      ];

      return (
        <div style={{ padding: '0 8px 8px' }}>
          <EuiInMemoryTable
            items={items}
            columns={innerColumns}
            compressed
            responsiveBreakpoint={false}
            pagination={items.length > 10 ? { pageSize: 10, showPerPageOptions: false } : undefined}
            search={
              items.length > 5
                ? { box: { incremental: true, placeholder: `Search ${row.label.toLowerCase()}...` } }
                : undefined
            }
            sorting={{ sort: { field: 'name', direction: 'asc' } }}
          />
          {/* Include history toggle */}
          {row.historyKey && sel.size > 0 && (
            <>
              <EuiSpacer size="m" />
              <EuiSwitch
                label="Include history"
                checked={history[row.historyKey]}
                onChange={() =>
                  setHistory((prev) => ({
                    ...prev,
                    [row.historyKey!]: !prev[row.historyKey!],
                  }))
                }
                compressed
              />
            </>
          )}
          {/* Secrets passphrase section */}
          {row.id === 'secrets' && sel.size > 0 && (
            <>
              <EuiSpacer size="m" />
              <EuiSwitch
                label="Include secret values"
                checked={includeSecretValues}
                onChange={() => setIncludeSecretValues((prev) => !prev)}
                compressed
              />
              {includeSecretValues && (
                <>
                  <EuiSpacer size="s" />
                  <EuiCallOut
                    title="This file will contain encrypted secret values. Keep it secure and remember your passphrase."
                    color="warning"
                    size="s"
                    iconType="warning"
                  />
                  <EuiSpacer size="s" />
                  <EuiFieldPassword
                    placeholder="Passphrase (min 8 characters)"
                    value={secretsPassphrase}
                    onChange={(e) => setSecretsPassphrase(e.target.value)}
                    type="dual"
                    compressed
                    fullWidth
                  />
                  <EuiSpacer size="xs" />
                  <EuiFieldPassword
                    placeholder="Repeat passphrase"
                    value={secretsPassphraseRepeat}
                    onChange={(e) => setSecretsPassphraseRepeat(e.target.value)}
                    type="dual"
                    compressed
                    fullWidth
                    isInvalid={secretsPassphraseRepeat.length > 0 && secretsPassphrase !== secretsPassphraseRepeat}
                  />
                </>
              )}
            </>
          )}
        </div>
      );
    },
    [
      allItems,
      selection,
      history,
      toggleAllInCategory,
      toggleItem,
      includeSecretValues,
      secretsPassphrase,
      secretsPassphraseRepeat,
    ],
  );

  const toggleExpanded = useCallback(
    (row: EntityRow) => {
      setExpandedRows((prev) => {
        const next = { ...prev };
        if (next[row.id]) {
          delete next[row.id];
        } else {
          next[row.id] = buildExpandedContent(row);
        }
        return next;
      });
    },
    [buildExpandedContent],
  );

  // Keep expanded rows in sync with selection/history changes.
  useEffect(() => {
    setExpandedRows((prev) => {
      const next: Record<string, ReactNode> = {};
      for (const key of Object.keys(prev)) {
        const row = ENTITY_ROWS.find((r) => r.id === key);
        if (row) {
          next[key] = buildExpandedContent(row);
        }
      }
      return next;
    });
  }, [buildExpandedContent]);

  const outerColumns: Array<EuiBasicTableColumn<EntityRow>> = useMemo(
    () => [
      {
        field: 'id',
        name: (
          <EuiCheckbox
            id="export-global-selectall"
            checked={allGloballySelected}
            indeterminate={!allGloballySelected && !noneGloballySelected}
            onChange={toggleAllGlobal}
          />
        ),
        width: '36px',
        render: (_id: string, row: EntityRow) => {
          if (row.id === 'settings') {
            return (
              <EuiCheckbox
                id="export-cat-settings"
                checked={includeSettings}
                onChange={() => setIncludeSettings((prev) => !prev)}
              />
            );
          }
          const category = row.id as EntityCategory;
          const items = allItems[category];
          const sel = selection[category];
          const allSelected = sel.size === items.length && items.length > 0;
          const someSelected = sel.size > 0 && sel.size < items.length;
          return (
            <EuiCheckbox
              id={`export-cat-${row.id}`}
              checked={allSelected}
              indeterminate={someSelected}
              onChange={() => toggleAllInCategory(category)}
            />
          );
        },
      },
      {
        field: 'label',
        name: 'Type',
        render: (_label: string, row: EntityRow) => (
          <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false}>
            <EuiFlexItem grow={false}>
              <EuiIcon type={row.icon} size="m" />
            </EuiFlexItem>
            <EuiFlexItem grow={false}>
              <strong>{row.label}</strong>
            </EuiFlexItem>
          </EuiFlexGroup>
        ),
      },
      {
        name: 'Selected',
        width: '80px',
        align: 'right' as const,
        render: (row: EntityRow) => {
          if (row.id === 'settings') {
            return null;
          }
          const category = row.id as EntityCategory;
          const items = allItems[category];
          const sel = selection[category];
          return (
            <EuiText size="s" color={sel.size === items.length ? 'success' : sel.size > 0 ? 'warning' : 'subdued'}>
              {sel.size}/{items.length}
            </EuiText>
          );
        },
      },
      {
        name: '',
        width: '40px',
        isExpander: true,
        render: (row: EntityRow) => {
          if (row.id === 'settings') {
            return null;
          }
          return (
            <EuiButtonIcon
              onClick={() => toggleExpanded(row)}
              aria-label={expandedRows[row.id] ? 'Collapse' : 'Expand'}
              iconType={expandedRows[row.id] ? 'arrowDown' : 'arrowRight'}
            />
          );
        },
      },
    ],
    [
      allGloballySelected,
      allItems,
      noneGloballySelected,
      selection,
      expandedRows,
      includeSettings,
      toggleAllInCategory,
      toggleExpanded,
      toggleAllGlobal,
    ],
  );

  return (
    <EuiModal onClose={onClose} style={{ width: 700, minHeight: 480 }}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>Export data</EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        {loading ? (
          <EuiFlexGroup justifyContent="center">
            <EuiFlexItem grow={false}>
              <EuiLoadingSpinner size="l" />
            </EuiFlexItem>
          </EuiFlexGroup>
        ) : (
          <EuiBasicTable
            items={visibleRows}
            responsiveBreakpoint={false}
            itemId="id"
            columns={outerColumns}
            itemIdToExpandedRowMap={expandedRows}
          />
        )}
      </EuiModalBody>
      <EuiModalFooter>
        <EuiButtonEmpty onClick={onClose}>Cancel</EuiButtonEmpty>
        <EuiButton
          onClick={handleExport}
          fill
          isLoading={exporting}
          disabled={
            !hasAnythingSelected ||
            loading ||
            (includeSecretValues && (secretsPassphrase.length < 8 || secretsPassphrase !== secretsPassphraseRepeat))
          }
          iconType="exportAction"
        >
          Export {hasAnythingSelected ? `(${totalSelected + (includeSettings ? 1 : 0)} items)` : ''}
        </EuiButton>
      </EuiModalFooter>
    </EuiModal>
  );
}
