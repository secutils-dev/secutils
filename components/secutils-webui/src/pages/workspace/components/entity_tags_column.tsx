import type { EuiBasicTableColumn } from '@elastic/eui';
import { EuiBadge, EuiFlexGroup, EuiFlexItem } from '@elastic/eui';

import type { EntityTag } from '../../../model';

export function getTagsColumn<T extends object>(width: string = '150px'): EuiBasicTableColumn<T> {
  return {
    field: 'tags',
    name: 'Tags',
    render: (tags?: EntityTag[]) => (
      <EuiFlexGroup gutterSize="xs" wrap responsive={false}>
        {tags?.map((tag) => (
          <EuiFlexItem grow={false} key={tag.id}>
            <EuiBadge color={tag.color}>{tag.name}</EuiBadge>
          </EuiFlexItem>
        ))}
      </EuiFlexGroup>
    ),
    width,
    sortable: false,
    truncateText: false,
  };
}
