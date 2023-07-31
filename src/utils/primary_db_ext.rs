mod raw_util;

use crate::{datastore::PrimaryDb, utils::Util};
use anyhow::bail;
use raw_util::RawUtil;
use sqlx::query_as;
use std::collections::HashMap;

/// Extends primary DB with the utility-related methods.
impl PrimaryDb {
    /// Retrieves all utils from the `Utils` table.
    pub async fn get_utils(&self) -> anyhow::Result<Vec<Util>> {
        let mut root_utils = query_as!(
            RawUtil,
            r#"
SELECT id, handle, name, keywords, parent_id
FROM utils
ORDER BY parent_id, id
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        // Utilities are sorted by the parent_id meaning that all root utilities are returned first.
        let child_utils = if let Some(position) = root_utils
            .iter()
            .position(|raw_util| raw_util.parent_id.is_some())
        {
            root_utils.split_off(position)
        } else {
            return root_utils.into_iter().map(Util::try_from).collect();
        };

        let mut parent_children_map = HashMap::<_, Vec<_>>::new();
        for util in child_utils {
            if let Some(parent_id) = util.parent_id {
                parent_children_map.entry(parent_id).or_default().push(util);
            } else {
                bail!("Child utility does not have a parent id.");
            }
        }

        root_utils
            .into_iter()
            .map(|root_util| Self::build_util_tree(root_util, &mut parent_children_map))
            .collect()
    }

    fn build_util_tree(
        raw_util: RawUtil,
        parent_children_map: &mut HashMap<i64, Vec<RawUtil>>,
    ) -> anyhow::Result<Util> {
        let utils = if let Some(mut children) = parent_children_map.remove(&raw_util.id) {
            Some(
                children
                    .drain(..)
                    .map(|util| Self::build_util_tree(util, parent_children_map))
                    .collect::<anyhow::Result<_>>()?,
            )
        } else {
            None
        };

        Util::try_from(raw_util).map(|util| Util { utils, ..util })
    }
}
