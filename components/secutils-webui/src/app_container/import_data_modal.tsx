import type { EuiBasicTableColumn } from '@elastic/eui';
import {
  EuiBadge,
  EuiBasicTable,
  EuiButton,
  EuiButtonEmpty,
  EuiButtonGroup,
  EuiButtonIcon,
  EuiCallOut,
  EuiCheckbox,
  EuiFieldPassword,
  EuiFilePicker,
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
  EuiSelect,
  EuiSpacer,
  EuiText,
} from '@elastic/eui';
import type { ReactNode } from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';

import type {
  ApplyDeletionSelections,
  ImportEntitySelection,
  ImportParams,
  ImportPreview,
} from '../model/user_data_export';
import { executeImport, previewImport } from '../model/user_data_export';
import type { PageToast } from '../pages/page';

interface Props {
  addToast: (toast: PageToast) => void;
  onClose: () => void;
  maxImportFileSize: number;
}

type Step = 'upload' | 'preview' | 'result';

interface EntityRowConfig {
  id: string;
  label: string;
  icon: string;
}

const ENTITY_ROW_CONFIGS: EntityRowConfig[] = [
  { id: 'settings', label: 'Settings', icon: 'gear' },
  { id: 'scripts', label: 'Scripts', icon: 'console' },
  { id: 'secrets', label: 'Secrets', icon: 'lock' },
  { id: 'responders', label: 'Responders', icon: 'node' },
  { id: 'certificateTemplates', label: 'Certificate Templates', icon: 'securityApp' },
  { id: 'privateKeys', label: 'Private Keys', icon: 'securityApp' },
  { id: 'contentSecurityPolicies', label: 'Content Security Policies', icon: 'globe' },
  { id: 'pageTrackers', label: 'Page Trackers', icon: 'cut' },
  { id: 'apiTrackers', label: 'API Trackers', icon: 'cut' },
];

interface ImportItem {
  id: string;
  name: string;
  hasConflict: boolean;
  /** Whether renaming can resolve this item's conflict. False for location+method conflicts. */
  renameAllowed: boolean;
}

export default function ImportDataModal({ addToast, onClose, maxImportFileSize }: Props) {
  const [step, setStep] = useState<Step>('upload');
  const [mode, setMode] = useState<'merge' | 'apply'>('merge');
  const [fileData, setFileData] = useState<unknown>(null);
  const [parseError, setParseError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [importResult, setImportResult] = useState<Record<
    string,
    { imported: number; updated: number; skipped: number; deleted: number; failed: number; errors: string[] }
  > | null>(null);
  const [secretsPassphrase, setSecretsPassphrase] = useState('');

  // Selection state for each entity type (items to import).
  const [selections, setSelections] = useState<Record<string, Map<string, ImportEntitySelection>>>({});
  // Deletion selections for Apply mode.
  const [deletionSelections, setDeletionSelections] = useState<Record<string, Set<string>>>({});
  // Expanded rows in the preview table.
  const [expandedRows, setExpandedRows] = useState<Record<string, ReactNode>>({});
  // Whether to import settings.
  const [importSettings, setImportSettings] = useState(true);

  const fileHasEncryptedSecrets = fileData != null && (fileData as Record<string, unknown>).secretsEncryption != null;

  const handleFileChange = useCallback(
    (files: FileList | null) => {
      setParseError(null);
      if (!files || files.length === 0) {
        setFileData(null);
        return;
      }
      const file = files[0];
      if (file.size > maxImportFileSize) {
        const maxSizeMb = (maxImportFileSize / (1024 * 1024)).toFixed(0);
        setParseError(`File is too large. Maximum size is ${maxSizeMb} MB.`);
        return;
      }

      const reader = new FileReader();
      reader.onload = () => {
        try {
          const parsed = JSON.parse(reader.result as string);
          if (!parsed.version || !parsed.data) {
            setParseError('Invalid file format: missing version or data fields.');
            return;
          }
          setFileData(parsed);
        } catch {
          setParseError('Failed to parse file as JSON.');
        }
      };
      reader.onerror = () => setParseError('Failed to read file.');
      reader.readAsText(file);
    },
    [maxImportFileSize],
  );

  const handlePreview = useCallback(async () => {
    if (!fileData) {
      return;
    }
    setLoading(true);
    try {
      const result = await previewImport({ data: fileData, mode });
      setPreview(result);
      if (!result.valid) {
        addToast({ id: 'import-invalid', color: 'warning', title: result.warnings[0] ?? 'Invalid import file.' });
        return;
      }
      // Initialize import selections: import everything by default.
      const newSelections: Record<string, Map<string, ImportEntitySelection>> = {};
      for (const [entityType, summary] of Object.entries(result.summary)) {
        if (!('total' in summary)) {
          continue;
        } // skip settings
        const map = new Map<string, ImportEntitySelection>();
        // Group all conflicts by sourceId so we can check rename eligibility across all conflicts for an item.
        const conflictsBySource = new Map<string, Array<{ sourceId: string; renameAllowed?: boolean }>>();
        for (const c of summary.conflicts ?? []) {
          const arr = conflictsBySource.get(c.sourceId) ?? [];
          arr.push(c);
          conflictsBySource.set(c.sourceId, arr);
        }
        const data = (fileData as Record<string, unknown>).data as Record<string, unknown[]>;
        const items = (data[entityType] ?? []) as Array<{ id: string; name: string }>;
        for (const item of items) {
          const conflicts = conflictsBySource.get(item.id);
          const canRename = conflicts ? conflicts.every((c) => c.renameAllowed !== false) : true;
          map.set(item.id, {
            sourceId: item.id,
            action: 'import',
            conflictResolution: conflicts ? (canRename ? 'rename' : 'overwrite') : undefined,
          });
        }
        newSelections[entityType] = map;
      }
      setSelections(newSelections);

      // Initialize settings import based on whether settings are included.
      setImportSettings(result.summary.settings?.included ?? false);

      // Initialize deletion selections for Apply mode (all unchecked by default).
      if (mode === 'apply' && result.toDelete) {
        const newDeletions: Record<string, Set<string>> = {};
        for (const entityType of Object.keys(result.toDelete)) {
          newDeletions[entityType] = new Set<string>();
        }
        setDeletionSelections(newDeletions);
      }

      setExpandedRows({});
      setStep('preview');
    } catch (err) {
      addToast({
        id: 'import-preview-error',
        color: 'danger',
        title: err instanceof Error ? err.message : 'Failed to preview import.',
      });
    } finally {
      setLoading(false);
    }
  }, [fileData, mode, addToast]);

  const handleImport = useCallback(async () => {
    if (!fileData || !preview) {
      return;
    }
    setImporting(true);
    try {
      let applyDeletions: ApplyDeletionSelections | undefined;
      if (mode === 'apply' && preview.toDelete) {
        applyDeletions = {
          scripts: Array.from(deletionSelections['scripts'] ?? []),
          secrets: Array.from(deletionSelections['secrets'] ?? []),
          responders: Array.from(deletionSelections['responders'] ?? []),
          certificateTemplates: Array.from(deletionSelections['certificateTemplates'] ?? []),
          privateKeys: Array.from(deletionSelections['privateKeys'] ?? []),
          contentSecurityPolicies: Array.from(deletionSelections['contentSecurityPolicies'] ?? []),
          pageTrackers: Array.from(deletionSelections['pageTrackers'] ?? []),
          apiTrackers: Array.from(deletionSelections['apiTrackers'] ?? []),
        };
      }

      const params: ImportParams = {
        data: fileData,
        mode,
        selections: {
          importSettings,
          ...Object.fromEntries(Object.entries(selections).map(([key, map]) => [key, Array.from(map.values())])),
        } as ImportParams['selections'],
        secretsPassphrase: fileHasEncryptedSecrets && secretsPassphrase ? secretsPassphrase : undefined,
        applyDeletions,
      };
      const result = await executeImport(params);
      const summary: Record<
        string,
        { imported: number; updated: number; skipped: number; deleted: number; failed: number; errors: string[] }
      > = {};
      for (const [key, res] of Object.entries(result.results)) {
        if (res.imported > 0 || res.updated > 0 || res.skipped > 0 || res.deleted > 0 || res.failed > 0) {
          summary[key] = {
            imported: res.imported,
            updated: res.updated,
            skipped: res.skipped,
            deleted: res.deleted,
            failed: res.failed,
            errors: res.errors ?? [],
          };
        }
      }
      setImportResult(summary);
      setStep('result');
      addToast({ id: 'import-success', color: 'success', title: 'Data imported successfully.' });
    } catch (err) {
      addToast({
        id: 'import-error',
        color: 'danger',
        title: err instanceof Error ? err.message : 'Failed to import data.',
      });
    } finally {
      setImporting(false);
    }
  }, [
    fileData,
    mode,
    preview,
    selections,
    deletionSelections,
    importSettings,
    secretsPassphrase,
    fileHasEncryptedSecrets,
    addToast,
  ]);

  const toggleEntitySelection = useCallback((entityType: string, sourceId: string) => {
    setSelections((prev) => {
      const map = new Map(prev[entityType]);
      const existing = map.get(sourceId);
      if (existing) {
        map.set(sourceId, { ...existing, action: existing.action === 'import' ? 'skip' : 'import' });
      }
      return { ...prev, [entityType]: map };
    });
  }, []);

  const toggleAllEntitySelections = useCallback((entityType: string) => {
    setSelections((prev) => {
      const map = new Map(prev[entityType]);
      const allImporting = Array.from(map.values()).every((s) => s.action === 'import');
      map.forEach((sel, id) => {
        map.set(id, { ...sel, action: allImporting ? 'skip' : 'import' });
      });
      return { ...prev, [entityType]: map };
    });
  }, []);

  const setConflictResolution = useCallback(
    (entityType: string, sourceId: string, resolution: 'rename' | 'overwrite' | 'skip') => {
      setSelections((prev) => {
        const map = new Map(prev[entityType]);
        const existing = map.get(sourceId);
        if (existing) {
          map.set(sourceId, { ...existing, conflictResolution: resolution });
        }
        return { ...prev, [entityType]: map };
      });
    },
    [],
  );

  const setAllConflictResolutions = useCallback(
    (resolution: 'rename' | 'overwrite' | 'skip') => {
      setSelections((prev) => {
        const next = { ...prev };
        for (const [entityType, map] of Object.entries(next)) {
          // Build a set of sourceIds that can't be renamed from preview conflicts.
          const nonRenameableIds = new Set<string>();
          if (preview) {
            const summary = preview.summary[entityType as keyof typeof preview.summary];
            if (summary && 'conflicts' in summary) {
              for (const c of summary.conflicts ?? []) {
                if (c.renameAllowed === false) {
                  nonRenameableIds.add(c.sourceId);
                }
              }
            }
          }
          const newMap = new Map(map);
          newMap.forEach((sel, id) => {
            if (sel.conflictResolution != null) {
              // If rename is requested but item doesn't allow it, fall back to overwrite.
              newMap.set(id, {
                ...sel,
                conflictResolution: resolution === 'rename' && nonRenameableIds.has(id) ? 'overwrite' : resolution,
              });
            }
          });
          next[entityType] = newMap;
        }
        return next;
      });
    },
    [preview],
  );

  const toggleDeletion = useCallback((entityType: string, id: string) => {
    setDeletionSelections((prev) => {
      const set = new Set(prev[entityType] ?? []);
      if (set.has(id)) {
        set.delete(id);
      } else {
        set.add(id);
      }
      return { ...prev, [entityType]: set };
    });
  }, []);

  const hasAnyConflicts =
    preview != null && Object.values(preview.summary).some((s) => 'conflicts' in s && (s.conflicts ?? []).length > 0);

  const hasNonRenameableConflicts = useMemo(() => {
    if (!preview) {
      return false;
    }
    for (const [entityType, summary] of Object.entries(preview.summary)) {
      if (!('conflicts' in summary)) {
        continue;
      }
      const entitySelections = selections[entityType];
      for (const c of summary.conflicts ?? []) {
        if (c.renameAllowed === false) {
          // Only count if the item is still selected for import.
          const sel = entitySelections?.get(c.sourceId);
          if (!sel || sel.action === 'import') {
            return true;
          }
        }
      }
    }
    return false;
  }, [preview, selections]);

  // Determine bulk conflict resolution state from all selected conflicting items.
  const bulkConflictResolution = useMemo((): string => {
    if (!hasAnyConflicts) {
      return '';
    }
    const resolutions = new Set<string>();
    for (const map of Object.values(selections)) {
      map.forEach((sel) => {
        if (sel.conflictResolution != null && sel.action === 'import') {
          resolutions.add(sel.conflictResolution);
        }
      });
    }
    if (resolutions.size === 1) {
      return Array.from(resolutions)[0];
    }
    return resolutions.size > 1 ? 'custom' : '';
  }, [selections, hasAnyConflicts]);

  // Helper to get items and conflict info for an entity type.
  const getEntityItems = useCallback(
    (entityType: string): ImportItem[] => {
      if (!preview || !fileData) {
        return [];
      }
      const summary = preview.summary[entityType as keyof typeof preview.summary];
      if (!summary) {
        return [];
      }
      const data = (fileData as Record<string, unknown>).data as Record<string, unknown[]>;
      const conflicts = 'conflicts' in summary ? (summary.conflicts ?? []) : [];
      return ((data[entityType] ?? []) as Array<{ id: string; name: string }>).map((item) => {
        const itemConflicts = conflicts.filter((c: { sourceId: string }) => c.sourceId === item.id);
        return {
          ...item,
          hasConflict: itemConflicts.length > 0,
          renameAllowed: itemConflicts.length === 0 || itemConflicts.every((c) => c.renameAllowed !== false),
        };
      });
    },
    [preview, fileData],
  );

  const getDeleteItems = useCallback(
    (entityType: string): Array<{ id: string; name: string }> => {
      if (!preview || !preview.toDelete || mode !== 'apply') {
        return [];
      }
      return (preview.toDelete[entityType as keyof typeof preview.toDelete] ?? []) as Array<{
        id: string;
        name: string;
      }>;
    },
    [preview, mode],
  );

  // Build expanded row content for import preview.
  const buildExpandedContent = useCallback(
    (entityType: string): ReactNode => {
      const items = getEntityItems(entityType);
      const entitySelections = selections[entityType] ?? new Map<string, ImportEntitySelection>();
      const deleteItems = getDeleteItems(entityType);

      const allImporting =
        items.length > 0 && Array.from(entitySelections.values()).every((s) => s.action === 'import');
      const someImporting = Array.from(entitySelections.values()).some((s) => s.action === 'import');

      const conflictMap = new Map<string, unknown>();
      if (preview) {
        const summary = preview.summary[entityType as keyof typeof preview.summary];
        if (summary && 'conflicts' in summary) {
          for (const c of summary.conflicts ?? []) {
            conflictMap.set(c.sourceId, c);
          }
        }
      }

      const innerColumns: Array<EuiBasicTableColumn<ImportItem>> = [
        {
          field: 'name',
          name: (
            <EuiCheckbox
              id={`import-inner-selectall-${entityType}`}
              label="Name"
              checked={allImporting}
              indeterminate={someImporting && !allImporting}
              onChange={() => toggleAllEntitySelections(entityType)}
            />
          ),
          render: (_name: string, item: ImportItem) => {
            const sel = entitySelections.get(item.id);
            return (
              <EuiFlexGroup gutterSize="s" alignItems="center" responsive={false}>
                <EuiFlexItem grow={false}>
                  <EuiCheckbox
                    id={`import-inner-${entityType}-${item.id}`}
                    label={item.name}
                    checked={sel?.action === 'import'}
                    onChange={() => toggleEntitySelection(entityType, item.id)}
                  />
                </EuiFlexItem>
                {item.hasConflict && sel?.action === 'import' && (
                  <EuiFlexItem grow={false}>
                    <EuiBadge color="warning">conflict</EuiBadge>
                  </EuiFlexItem>
                )}
              </EuiFlexGroup>
            );
          },
        },
        {
          name: 'Resolution',
          width: '130px',
          render: (item: ImportItem) => {
            const sel = entitySelections.get(item.id);
            if (!item.hasConflict || sel?.action !== 'import') {
              return null;
            }
            const options = item.renameAllowed
              ? [
                  { value: 'rename', text: 'Rename' },
                  { value: 'overwrite', text: 'Overwrite' },
                  { value: 'skip', text: 'Skip' },
                ]
              : [
                  { value: 'overwrite', text: 'Overwrite' },
                  { value: 'skip', text: 'Skip' },
                ];
            return (
              <EuiSelect
                compressed
                options={options}
                value={sel.conflictResolution ?? (item.renameAllowed ? 'rename' : 'overwrite')}
                onChange={(e) =>
                  setConflictResolution(entityType, item.id, e.target.value as 'rename' | 'overwrite' | 'skip')
                }
              />
            );
          },
        },
      ];

      // Only show Resolution column if there are conflicts in this entity type.
      const hasConflicts = items.some((i) => i.hasConflict);
      const displayColumns = hasConflicts ? innerColumns : [innerColumns[0]];

      return (
        <div style={{ padding: '0 8px 8px' }}>
          {items.length > 0 && (
            <EuiInMemoryTable
              items={items}
              columns={displayColumns}
              compressed
              responsiveBreakpoint={false}
              pagination={items.length > 10 ? { pageSize: 10, showPerPageOptions: false } : undefined}
              sorting={{ sort: { field: 'name', direction: 'asc' } }}
            />
          )}
          {deleteItems.length > 0 && (
            <>
              {items.length > 0 && <EuiSpacer size="s" />}
              <EuiCallOut title={`${deleteItems.length} item(s) not in file`} color="danger" size="s" iconType="trash">
                <EuiText size="xs" color="subdued">
                  Check items you want to delete. Unchecked items will be kept.
                </EuiText>
                <EuiSpacer size="xs" />
                {deleteItems.map((item: { id: string; name: string }) => (
                  <EuiCheckbox
                    key={item.id}
                    id={`delete-${entityType}-${item.id}`}
                    label={item.name}
                    checked={(deletionSelections[entityType] ?? new Set()).has(item.id)}
                    onChange={() => toggleDeletion(entityType, item.id)}
                  />
                ))}
              </EuiCallOut>
            </>
          )}
        </div>
      );
    },
    [
      getEntityItems,
      getDeleteItems,
      selections,
      deletionSelections,
      preview,
      toggleAllEntitySelections,
      toggleEntitySelection,
      setConflictResolution,
      toggleDeletion,
    ],
  );

  const toggleExpanded = useCallback(
    (entityType: string) => {
      setExpandedRows((prev) => {
        const next = { ...prev };
        if (next[entityType]) {
          delete next[entityType];
        } else {
          next[entityType] = buildExpandedContent(entityType);
        }
        return next;
      });
    },
    [buildExpandedContent],
  );

  // Keep expanded rows in sync with state changes.
  useEffect(() => {
    setExpandedRows((prev) => {
      const next: Record<string, ReactNode> = {};
      for (const key of Object.keys(prev)) {
        next[key] = buildExpandedContent(key);
      }
      return next;
    });
  }, [buildExpandedContent]);

  // Visible entity rows for import preview.
  const visibleRows = useMemo(() => {
    if (!preview) {
      return [];
    }
    return ENTITY_ROW_CONFIGS.filter((r) => {
      if (r.id === 'settings') {
        return preview.summary.settings?.included ?? false;
      }
      const summary = preview.summary[r.id as keyof typeof preview.summary];
      const hasItems = summary && 'total' in summary && summary.total > 0;
      const hasDeletes = getDeleteItems(r.id).length > 0;
      return hasItems || hasDeletes;
    });
  }, [preview, getDeleteItems]);

  const selectedCountForType = useCallback(
    (entityType: string): number => {
      const map = selections[entityType];
      if (!map) {
        return 0;
      }
      let count = 0;
      map.forEach((sel) => {
        if (sel.action === 'import') {
          count++;
        }
      });
      return count;
    },
    [selections],
  );

  const totalForType = useCallback(
    (entityType: string): number => {
      if (!preview) {
        return 0;
      }
      const summary = preview.summary[entityType as keyof typeof preview.summary];
      return summary && 'total' in summary ? summary.total : 0;
    },
    [preview],
  );

  const importTotalSelected = useMemo(() => {
    let count = 0;
    for (const map of Object.values(selections)) {
      map.forEach((sel) => {
        if (sel.action === 'import') {
          count++;
        }
      });
    }
    return count;
  }, [selections]);

  const importTotalAvailable = useMemo(() => {
    let count = 0;
    for (const map of Object.values(selections)) {
      count += map.size;
    }
    return count;
  }, [selections]);

  const hasSettingsInFile = preview?.summary.settings?.included ?? false;
  const allGloballySelected = importTotalSelected === importTotalAvailable && (!hasSettingsInFile || importSettings);
  const noneGloballySelected = importTotalSelected === 0 && (!hasSettingsInFile || !importSettings);

  const toggleAllGlobalImport = useCallback(() => {
    const shouldSelectAll = !allGloballySelected;
    if (hasSettingsInFile) {
      setImportSettings(shouldSelectAll);
    }
    setSelections((prev) => {
      const newAction = shouldSelectAll ? 'import' : 'skip';
      const next: Record<string, Map<string, ImportEntitySelection>> = {};
      for (const [key, map] of Object.entries(prev)) {
        const newMap = new Map(map);
        newMap.forEach((sel, id) => {
          newMap.set(id, { ...sel, action: newAction });
        });
        next[key] = newMap;
      }
      return next;
    });
  }, [allGloballySelected, hasSettingsInFile]);

  const outerColumns: Array<EuiBasicTableColumn<EntityRowConfig>> = useMemo(
    () => [
      {
        field: 'id',
        name: (
          <EuiCheckbox
            id="import-global-selectall"
            checked={allGloballySelected}
            indeterminate={!allGloballySelected && !noneGloballySelected}
            onChange={toggleAllGlobalImport}
          />
        ),
        width: '36px',
        render: (_id: string, row: EntityRowConfig) => {
          if (row.id === 'settings') {
            return (
              <EuiCheckbox
                id="import-cat-settings"
                checked={importSettings}
                onChange={() => setImportSettings((prev) => !prev)}
              />
            );
          }
          const total = totalForType(row.id);
          const selected = selectedCountForType(row.id);
          if (total === 0) {
            return null;
          }
          return (
            <EuiCheckbox
              id={`import-cat-${row.id}`}
              checked={selected === total && total > 0}
              indeterminate={selected > 0 && selected < total}
              onChange={() => toggleAllEntitySelections(row.id)}
            />
          );
        },
      },
      {
        field: 'label',
        name: 'Type',
        render: (_label: string, row: EntityRowConfig) => (
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
        name: 'Items',
        width: '80px',
        align: 'right' as const,
        render: (row: EntityRowConfig) => {
          if (row.id === 'settings') {
            return preview?.summary.settings?.hasExisting && importSettings ? (
              <EuiBadge color="warning">will overwrite</EuiBadge>
            ) : null;
          }
          const total = totalForType(row.id);
          const selected = selectedCountForType(row.id);
          if (total === 0) {
            const deleteCount = getDeleteItems(row.id).length;
            return deleteCount > 0 ? <EuiBadge color="danger">{deleteCount} to delete</EuiBadge> : null;
          }
          return (
            <EuiText size="s" color={selected === total ? 'success' : selected > 0 ? 'warning' : 'subdued'}>
              {selected}/{total}
            </EuiText>
          );
        },
      },
      {
        name: '',
        width: '40px',
        isExpander: true,
        render: (row: EntityRowConfig) => {
          if (row.id === 'settings') {
            return null;
          }
          return (
            <EuiButtonIcon
              onClick={() => toggleExpanded(row.id)}
              aria-label={expandedRows[row.id] ? 'Collapse' : 'Expand'}
              iconType={expandedRows[row.id] ? 'arrowDown' : 'arrowRight'}
            />
          );
        },
      },
    ],
    [
      allGloballySelected,
      noneGloballySelected,
      totalForType,
      selectedCountForType,
      getDeleteItems,
      expandedRows,
      importSettings,
      preview,
      toggleAllEntitySelections,
      toggleExpanded,
      toggleAllGlobalImport,
    ],
  );

  let content;
  if (step === 'upload') {
    content = (
      <>
        <EuiText size="s" color="subdued">
          Upload a <code>.secutils.json</code> export file.
        </EuiText>
        <EuiSpacer size="m" />
        <EuiFilePicker
          accept=".json,.secutils.json"
          fullWidth
          onChange={handleFileChange}
          display="large"
          initialPromptText="Drop a file or click to upload"
        />
        {parseError && (
          <>
            <EuiSpacer size="s" />
            <EuiCallOut title={parseError} color="danger" size="s" />
          </>
        )}
        {fileHasEncryptedSecrets && (
          <>
            <EuiSpacer size="s" />
            <EuiCallOut title="This file contains encrypted secret values." color="primary" size="s" iconType="lock">
              <EuiFieldPassword
                placeholder="Enter passphrase to decrypt secrets"
                value={secretsPassphrase}
                onChange={(e) => setSecretsPassphrase(e.target.value)}
                type="dual"
                compressed
                fullWidth
              />
            </EuiCallOut>
          </>
        )}
        <EuiSpacer size="m" />
        <EuiText size="s">
          <strong>Import mode</strong>
        </EuiText>
        <EuiSpacer size="s" />
        <EuiButtonGroup
          legend="Import mode"
          options={[
            { id: 'merge', label: 'Merge' },
            { id: 'apply', label: 'Apply' },
          ]}
          idSelected={mode}
          onChange={(id) => setMode(id as 'merge' | 'apply')}
        />
        <EuiSpacer size="xs" />
        <EuiText size="xs" color="subdued">
          {mode === 'merge'
            ? 'Add items from the file to your existing data. Existing items are kept.'
            : 'Make your data match the file exactly. Items not in the file can be removed.'}
        </EuiText>
      </>
    );
  } else if (step === 'preview' && preview) {
    content = (
      <>
        {(preview.warnings ?? []).map((w, i) => (
          <EuiCallOut key={i} title={w} color="warning" size="s" iconType="warning" />
        ))}
        {(preview.warnings ?? []).length > 0 && <EuiSpacer size="s" />}

        {hasAnyConflicts && (
          <>
            <EuiText size="xs">
              <strong>Resolve all conflicts:</strong>
            </EuiText>
            <EuiSpacer size="xs" />
            <EuiButtonGroup
              legend="Bulk conflict resolution"
              options={[
                { id: 'rename', label: 'Rename', isDisabled: hasNonRenameableConflicts },
                { id: 'overwrite', label: 'Overwrite' },
                { id: 'skip', label: 'Skip' },
                { id: 'custom', label: 'Custom', isDisabled: true },
              ]}
              idSelected={bulkConflictResolution}
              onChange={(id) => {
                if (id !== 'custom') {
                  setAllConflictResolutions(id as 'rename' | 'overwrite' | 'skip');
                }
              }}
              buttonSize="compressed"
              type="single"
            />
            {hasNonRenameableConflicts && (
              <>
                <EuiSpacer size="xs" />
                <EuiText size="xs" color="subdued">
                  Some conflicts cannot be resolved by renaming.
                </EuiText>
              </>
            )}
            <EuiSpacer size="s" />
          </>
        )}

        {visibleRows.length === 0 ? (
          <EuiText size="s" color="subdued">
            No items found in the import file.
          </EuiText>
        ) : (
          visibleRows.length > 0 && (
            <EuiBasicTable
              items={visibleRows}
              itemId="id"
              responsiveBreakpoint={false}
              columns={outerColumns}
              itemIdToExpandedRowMap={expandedRows}
            />
          )
        )}
      </>
    );
  } else if (step === 'result' && importResult) {
    const entityTypeLabels: Record<string, string> = { settings: 'Settings' };
    for (const r of ENTITY_ROW_CONFIGS) {
      entityTypeLabels[r.id] = r.label;
    }
    content = (
      <>
        <EuiCallOut title="Import complete" color="success" iconType="check">
          {Object.entries(importResult).map(([key, res]) => (
            <p key={key}>
              <strong>{entityTypeLabels[key] ?? key}</strong>: {res.imported} imported
              {res.updated > 0 ? `, ${res.updated} updated` : ''}
              {res.skipped > 0 ? `, ${res.skipped} skipped` : ''}
              {res.deleted > 0 ? `, ${res.deleted} deleted` : ''}
              {res.failed > 0 ? `, ${res.failed} failed` : ''}
            </p>
          ))}
          {Object.keys(importResult).length === 0 && <p>No changes made.</p>}
        </EuiCallOut>
        {Object.entries(importResult).some(([, res]) => res.errors.length > 0) && (
          <>
            <EuiSpacer size="s" />
            <EuiCallOut title="Errors" color="danger" size="s" iconType="warning">
              {Object.entries(importResult)
                .filter(([, res]) => res.errors.length > 0)
                .map(([key, res]) => (
                  <div key={key}>
                    {res.errors.map((err, i) => (
                      <p key={i}>{err}</p>
                    ))}
                  </div>
                ))}
            </EuiCallOut>
          </>
        )}
      </>
    );
  }

  return (
    <EuiModal onClose={onClose} style={{ width: 700, minHeight: 480 }}>
      <EuiModalHeader>
        <EuiModalHeaderTitle>
          {step === 'upload' ? 'Import data' : step === 'preview' ? 'Review import' : 'Import results'}
        </EuiModalHeaderTitle>
      </EuiModalHeader>
      <EuiModalBody>
        {loading ? (
          <EuiFlexGroup justifyContent="center">
            <EuiFlexItem grow={false}>
              <EuiLoadingSpinner size="l" />
            </EuiFlexItem>
          </EuiFlexGroup>
        ) : (
          content
        )}
      </EuiModalBody>
      <EuiModalFooter>
        {step === 'upload' && (
          <>
            <EuiButtonEmpty onClick={onClose}>Cancel</EuiButtonEmpty>
            <EuiButton
              onClick={handlePreview}
              fill
              disabled={!fileData || !!parseError || (fileHasEncryptedSecrets && secretsPassphrase.length < 8)}
              isLoading={loading}
            >
              Preview
            </EuiButton>
          </>
        )}
        {step === 'preview' && (
          <>
            <EuiButtonEmpty onClick={() => setStep('upload')}>Back</EuiButtonEmpty>
            <EuiButton
              onClick={handleImport}
              fill
              color={mode === 'apply' ? 'danger' : 'primary'}
              isLoading={importing}
              iconType="importAction"
            >
              {mode === 'apply' ? 'Apply' : 'Import'}
            </EuiButton>
          </>
        )}
        {step === 'result' && (
          <EuiButton onClick={onClose} fill>
            Done
          </EuiButton>
        )}
      </EuiModalFooter>
    </EuiModal>
  );
}
