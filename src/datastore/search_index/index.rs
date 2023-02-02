use crate::{
    datastore::{commit_index, SearchFilter, SearchIndexSchemaFields},
    search::SearchItem,
    users::UserId,
};
use anyhow::{bail, Context};
use std::{collections::HashMap, thread, time::Duration};
use tantivy::{
    collector::TopDocs,
    directory::error::LockError,
    error::TantivyError,
    query::{Query, TermQuery},
    schema::*,
    Index, IndexReader, IndexWriter,
};
use time::OffsetDateTime;

fn entity_to_document(
    entity: &SearchItem,
    schema_fields: &SearchIndexSchemaFields,
) -> anyhow::Result<Document> {
    let mut doc = Document::default();

    doc.add_u64(schema_fields.id, entity.id);
    doc.add_i64(
        schema_fields.user_id,
        entity.user_id.unwrap_or_else(UserId::empty).0,
    );

    doc.add_text(schema_fields.label, &entity.label);
    doc.add_text(schema_fields.category, &entity.category);
    if let Some(ref keywords) = entity.keywords {
        doc.add_text(schema_fields.keywords, keywords);
    }
    if let Some(ref sub_category) = entity.sub_category {
        doc.add_text(schema_fields.sub_category, sub_category);
    }

    if let Some(ref meta) = entity.meta {
        doc.add_bytes(schema_fields.meta, serde_json::ser::to_vec(meta)?);
    }

    doc.add_date(
        schema_fields.timestamp,
        tantivy::DateTime::from_utc(entity.timestamp),
    );

    Ok(doc)
}

#[derive(Clone)]
pub struct SearchIndex {
    schema_fields: SearchIndexSchemaFields,
    index: Index,
    index_reader: IndexReader,
}

impl SearchIndex {
    /// Opens `Search` index.
    pub fn open<I: FnOnce(Schema) -> anyhow::Result<(Index, IndexReader)>>(
        initializer: I,
    ) -> anyhow::Result<Self> {
        let (schema_fields, schema) = SearchIndexSchemaFields::build();
        let (index, index_reader) = initializer(schema)?;

        Ok(Self {
            schema_fields,
            index,
            index_reader,
        })
    }

    /// Retrieves item from the `Search` index using id.
    pub fn get(&self, id: u64) -> anyhow::Result<Option<SearchItem>> {
        let handle_query: Box<dyn Query> = Box::new(TermQuery::new(
            Term::from_field_u64(self.schema_fields.id, id),
            IndexRecordOption::Basic,
        )) as Box<dyn Query>;

        self.execute_query(handle_query).and_then(|mut entities| {
            if entities.is_empty() {
                return Ok(None);
            }

            if entities.len() > 1 {
                bail!(
                    "Founds {} items for the same id {}.",
                    entities.len().to_string(),
                    id
                );
            }

            Ok(Some(entities.remove(0)))
        })
    }

    /// Search for items using the specified filter.
    pub fn search(&self, filter: SearchFilter) -> anyhow::Result<Vec<SearchItem>> {
        self.execute_query(filter.into_query(&self.index, &self.schema_fields)?)
    }

    /// Inserts or updates search item in the `Search` index.
    pub fn upsert<I: AsRef<SearchItem>>(&self, item: I) -> anyhow::Result<()> {
        let item = item.as_ref();
        let existing_item = self.get(item.id)?;

        let mut index_writer = self.acquire_index_writer()?;
        if existing_item.is_some() {
            index_writer.delete_term(Term::from_field_u64(self.schema_fields.id, item.id));
        }

        index_writer.add_document(entity_to_document(item, &self.schema_fields)?)?;

        commit_index(&mut index_writer, &self.index_reader)
    }

    fn acquire_index_writer(&self) -> anyhow::Result<IndexWriter> {
        loop {
            match self.index.writer(3_000_000) {
                Ok(writer) => break Ok(writer),
                Err(TantivyError::LockFailure(LockError::LockBusy, reason)) => {
                    log::warn!(
                        "Failed to get user index writer lock, will re-try in 50ms: {:?}",
                        reason
                    );
                    thread::sleep(Duration::from_millis(50));
                }
                Err(err) => {
                    bail!(err)
                }
            };
        }
    }

    fn execute_query(&self, query: impl Query) -> anyhow::Result<Vec<SearchItem>> {
        let searcher = self.index_reader.searcher();

        let collector = TopDocs::with_limit(10000);
        let top_docs = searcher.search(&query, &collector)?;

        let mut found_docs = Vec::with_capacity(top_docs.len());
        for (_, doc_address) in top_docs.into_iter() {
            let doc = searcher
                .doc(doc_address)
                .with_context(|| "Failed to retrieve search hit document.".to_string())?;

            let mut id: Option<u64> = None;
            let mut user_id: Option<UserId> = None;
            let mut label: Option<String> = None;
            let mut category: Option<String> = None;
            let mut sub_category: Option<String> = None;
            let mut meta: Option<HashMap<String, String>> = None;
            let mut timestamp: Option<OffsetDateTime> = None;
            for field_value in doc {
                if field_value.field == self.schema_fields.id {
                    if let Value::U64(field_value_content) = field_value.value {
                        id.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.user_id {
                    if let Value::I64(field_value_content) = field_value.value {
                        if field_value_content != UserId::empty().0 {
                            user_id.replace(UserId(field_value_content));
                        }
                    }
                } else if field_value.field == self.schema_fields.label {
                    if let Value::Str(field_value_content) = field_value.value {
                        label.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.category {
                    if let Value::Str(field_value_content) = field_value.value {
                        category.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.sub_category {
                    if let Value::Str(field_value_content) = field_value.value {
                        sub_category.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.meta {
                    if let Value::Bytes(field_value_content) = field_value.value {
                        meta.replace(serde_json::from_slice::<HashMap<_, _>>(
                            &field_value_content,
                        )?);
                    }
                } else if field_value.field == self.schema_fields.timestamp {
                    if let Value::Date(field_value_content) = field_value.value {
                        timestamp.replace(field_value_content.into_utc());
                    }
                }
            }

            match (id, label, category, timestamp) {
                (Some(id), Some(label), Some(category), Some(timestamp))
                    if !label.is_empty() && !category.is_empty() =>
                {
                    found_docs.push(SearchItem {
                        id,
                        user_id,
                        label,
                        keywords: None,
                        category,
                        sub_category,
                        meta,
                        timestamp,
                    })
                }
                _ => {}
            }
        }

        Ok(found_docs)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        datastore::{SearchFilter, SearchIndex},
        tests::{open_index, MockSearchItemBuilder},
        users::UserId,
    };
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn can_index_and_retrieve_items() -> anyhow::Result<()> {
        let index = SearchIndex::open(open_index)?;
        assert_eq!(index.get(1)?, None);

        let items = vec![
            MockSearchItemBuilder::new(
                1,
                "some-label",
                "some-category",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .build(),
            MockSearchItemBuilder::new(
                2,
                "other-label",
                "other-category",
                // January 1, 2010 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .set_keywords("some keywords")
            .set_user_id(UserId(3))
            .set_sub_category("some-handle")
            .set_meta([("one".to_string(), "two".to_string())])
            .build(),
        ];
        for item in items {
            index.upsert(item)?;
        }

        assert_debug_snapshot!(index.get(1)?, @r###"
        Some(
            SearchItem {
                id: 1,
                label: "some-label",
                keywords: None,
                category: "some-category",
                sub_category: None,
                user_id: None,
                meta: None,
                timestamp: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(index.get(2)?, @r###"
        Some(
            SearchItem {
                id: 2,
                label: "other-label",
                keywords: None,
                category: "other-category",
                sub_category: Some(
                    "some-handle",
                ),
                user_id: Some(
                    UserId(
                        3,
                    ),
                ),
                meta: Some(
                    {
                        "one": "two",
                    },
                ),
                timestamp: 2010-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_eq!(index.get(3)?, None);

        Ok(())
    }

    #[test]
    fn can_update_item() -> anyhow::Result<()> {
        let index = SearchIndex::open(open_index)?;

        index.upsert(
            MockSearchItemBuilder::new(
                1,
                "some-label",
                "some-category",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .build(),
        )?;
        assert_debug_snapshot!(index.get(1)?, @r###"
        Some(
            SearchItem {
                id: 1,
                label: "some-label",
                keywords: None,
                category: "some-category",
                sub_category: None,
                user_id: None,
                meta: None,
                timestamp: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);

        index.upsert(
            MockSearchItemBuilder::new(
                1,
                "other-label",
                "other-category",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .set_sub_category("Some-Sub-Category")
            .build(),
        )?;
        assert_debug_snapshot!(index.get(1)?, @r###"
        Some(
            SearchItem {
                id: 1,
                label: "other-label",
                keywords: None,
                category: "other-category",
                sub_category: Some(
                    "Some-Sub-Category",
                ),
                user_id: None,
                meta: None,
                timestamp: 2010-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);

        Ok(())
    }

    #[test]
    fn can_search() -> anyhow::Result<()> {
        let index = SearchIndex::open(open_index)?;
        let item_user_3 = MockSearchItemBuilder::new(
            1,
            "some-label",
            "some-category",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_user_id(UserId(3))
        .build();
        let item_user_4 = MockSearchItemBuilder::new(
            2,
            "other-label",
            "other-category",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_user_id(UserId(4))
        .build();

        let public_item = MockSearchItemBuilder::new(
            3,
            "public-label",
            "public-category",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build();

        index.upsert(&item_user_3)?;
        index.upsert(&item_user_4)?;
        index.upsert(&public_item)?;

        let mut public_items = index.search(SearchFilter::default())?;
        public_items.sort_by(|item_a, item_b| item_a.id.cmp(&item_b.id));
        assert_eq!(public_items, vec![public_item.clone()]);

        let mut public_and_user_items =
            index.search(SearchFilter::default().with_user_id(UserId(3)))?;
        public_and_user_items.sort_by(|item_a, item_b| item_a.id.cmp(&item_b.id));
        assert_eq!(
            public_and_user_items,
            vec![item_user_3, public_item.clone()]
        );

        let mut public_and_user_items =
            index.search(SearchFilter::default().with_user_id(UserId(4)))?;
        public_and_user_items.sort_by(|item_a, item_b| item_a.id.cmp(&item_b.id));
        assert_eq!(
            public_and_user_items,
            vec![item_user_4, public_item.clone()]
        );

        assert_eq!(
            index.search(SearchFilter::default().with_user_id(UserId(5)))?,
            vec![public_item]
        );

        Ok(())
    }
}
