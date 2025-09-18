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
import { useCallback, useState } from 'react';
import { useNavigate } from 'react-router';

import type { AsyncData, SearchItem, SerializedSearchItem } from '../../../model';
import { deserializeSearchItem, getApiRequestConfig, getApiUrl, getErrorMessage, ResponseError } from '../../../model';
import { getUtilIcon } from '../utils';

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

  const [searchItems, setSearchItems] = useState<AsyncData<EuiSelectableTemplateSitewideOption[]> | null>(null);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  const onKeyUpCapture = useCallback(
    debounce((searchQuery: string) => {
      if (!searchQuery) {
        setSearchItems({ status: 'succeeded', data: [] });
        return;
      }

      fetch(getApiUrl('/api/search'), {
        ...getApiRequestConfig('POST'),
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
              const icon =
                searchItem.category === 'Utils' ? getUtilIcon(searchItem.meta?.handle ?? '', 'search') : undefined;
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
      searchProps={{ onKeyUpCapture }}
      popoverProps={{ width: isWithinMaxBreakpoint ? undefined : 400 }}
      popoverFooter={
        <EuiText color="subdued" size="xs">
          <EuiFlexGroup alignItems="center" gutterSize="s" responsive={false} wrap>
            <EuiFlexItem />
            <EuiFlexItem grow={false}>Quickly search using</EuiFlexItem>
            <EuiFlexItem grow={false}>
              <EuiBadge>Command + K</EuiBadge>
            </EuiFlexItem>
          </EuiFlexGroup>
        </EuiText>
      }
    />
  );
}
