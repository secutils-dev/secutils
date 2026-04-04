import type { EuiSelectableTemplateSitewideOption } from '@elastic/eui';
import {
  EuiBadge,
  EuiFlexGroup,
  EuiFlexItem,
  EuiSelectableTemplateSitewide,
  EuiText,
  useIsWithinMaxBreakpoint,
} from '@elastic/eui';
import type { KeyboardEvent } from 'react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router';

import type { AsyncData, SearchItem, SerializedSearchItem } from '../../../model';
import { apiFetch, deserializeSearchItem, getErrorMessage, ResponseError } from '../../../model';
import { getUtilIcon } from '../utils';

function isMacLikePlatform(): boolean {
  if (typeof navigator === 'undefined') {
    return false;
  }
  return /Mac|iPhone|iPad|iPod/i.test(navigator.platform) || navigator.userAgent.includes('Mac OS');
}

function debounce(callback: (searchQuery: string) => void) {
  let timeout: number;
  return (e: KeyboardEvent<HTMLInputElement>) => {
    const searchQuery = e.currentTarget.value;
    window.clearTimeout(timeout);
    timeout = window.setTimeout(() => callback(searchQuery), 100);
  };
}

export function SiteSearchBar() {
  const navigate = useNavigate();
  const isWithinMaxBreakpoint = useIsWithinMaxBreakpoint('s');
  const searchInputRef = useRef<HTMLInputElement | null>(null);

  const [searchItems, setSearchItems] = useState<AsyncData<EuiSelectableTemplateSitewideOption[]> | null>(null);
  const [shortcutHint, setShortcutHint] = useState('⌘K');

  useEffect(() => {
    setShortcutHint(isMacLikePlatform() ? '⌘K' : 'Ctrl+K');
  }, []);

  useEffect(() => {
    const onGlobalKeyDown = (e: globalThis.KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== 'k') {
        return;
      }
      const t = e.target;
      if (t instanceof Element && t.closest('.monaco-editor')) {
        return;
      }
      e.preventDefault();
      searchInputRef.current?.focus();
    };
    window.addEventListener('keydown', onGlobalKeyDown);
    return () => window.removeEventListener('keydown', onGlobalKeyDown);
  }, []);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const onKeyUpCapture = useCallback(
    debounce((searchQuery: string) => {
      if (!searchQuery) {
        setSearchItems({ status: 'succeeded', data: [] });
        return;
      }

      apiFetch('/api/search', {
        method: 'POST',
        body: JSON.stringify({ query: searchQuery }),
      })
        .then(async (res) => {
          if (!res.ok) {
            throw await ResponseError.fromResponse(res);
          }

          const searchItems = (await res.json()) as SerializedSearchItem[];
          setSearchItems({
            status: 'succeeded',
            data: searchItems.map((serializedSearchItem) => {
              const searchItem = deserializeSearchItem(serializedSearchItem);
              const icon = searchItem.category === 'Utils' ? getUtilIcon(searchItem.meta?.handle ?? '') : undefined;
              return {
                label: searchItem.label,
                icon: icon ? { type: icon } : undefined,
                meta: [{ text: searchItem.category, type: 'application', highlightSearchString: true }],
                item: searchItem,
              };
            }),
          });
        })
        .catch((err: Error) => {
          setSearchItems({ status: 'failed', error: getErrorMessage(err) });
        });
    }),
    [],
  );

  const onChange = (updatedOptions: EuiSelectableTemplateSitewideOption[]) => {
    const clickedItem = updatedOptions.find((option) => option.checked === 'on');
    if (clickedItem?.item) {
      const searchItem = clickedItem?.item as SearchItem;
      if (searchItem.meta?.handle) {
        navigate(`/ws/${searchItem.meta.handle}`);
      }
    }
  };

  return (
    <EuiSelectableTemplateSitewide
      isPreFiltered
      isLoading={searchItems?.status === 'pending'}
      onChange={onChange}
      options={searchItems?.status === 'succeeded' ? searchItems.data : []}
      searchProps={{
        onKeyUpCapture,
        inputRef: (node: HTMLInputElement | null) => {
          searchInputRef.current = node;
        },
      }}
      popoverProps={{ width: isWithinMaxBreakpoint ? undefined : 400 }}
      popoverFooter={
        <EuiText color="subdued" size="xs">
          <EuiFlexGroup alignItems="center" gutterSize="s" responsive={false} wrap>
            <EuiFlexItem />
            <EuiFlexItem grow={false}>Quickly search using</EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiBadge>{shortcutHint}</EuiBadge>
            </EuiFlexItem>
          </EuiFlexGroup>
        </EuiText>
      }
    />
  );
}
