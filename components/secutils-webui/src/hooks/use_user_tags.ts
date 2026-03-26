import { useCallback, useEffect, useState } from 'react';

import type { UserTag } from '../model/user_tags';
import { getUserTags } from '../model/user_tags';

export function useUserTags() {
  const [allTags, setAllTags] = useState<UserTag[]>([]);

  const refreshTags = useCallback(() => {
    getUserTags()
      .then(setAllTags)
      .catch(() => {});
  }, []);

  useEffect(() => {
    refreshTags();
  }, [refreshTags]);

  return { allTags, setAllTags, refreshTags };
}
