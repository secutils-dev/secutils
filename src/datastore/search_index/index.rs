use crate::{
    datastore::{commit_index, SearchFilter, SearchIndexSchemaFields},
    search::SearchItem,
};
use anyhow::{bail, Context};
use std::{thread, time::Duration};
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

    doc.add_text(schema_fields.id, &entity.id);
    doc.add_text(schema_fields.content, &entity.content);
    doc.add_date(
        schema_fields.timestamp,
        tantivy::DateTime::from_utc(entity.timestamp),
    );

    if let Some(ref user_handle) = entity.user_handle {
        doc.add_text(schema_fields.user_handle, user_handle);
    }

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
    pub fn get<T: AsRef<str>>(&self, id: T) -> anyhow::Result<Option<SearchItem>> {
        let handle_query: Box<dyn Query> = Box::new(TermQuery::new(
            Term::from_field_text(self.schema_fields.id, &id.as_ref().to_lowercase()),
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
                    id.as_ref()
                );
            }

            Ok(Some(entities.remove(0)))
        })
    }

    /// Search for items using the specified filter.
    pub fn search(&self, filter: SearchFilter) -> anyhow::Result<Vec<SearchItem>> {
        self.execute_query(filter.into_query(&self.schema_fields))
    }

    /// Inserts or updates search item in the `Search` index.
    pub fn upsert<I: AsRef<SearchItem>>(&self, item: I) -> anyhow::Result<()> {
        let item = item.as_ref();
        let existing_item = self.get(&item.id)?;

        let mut index_writer = self.acquire_index_writer()?;
        if existing_item.is_some() {
            index_writer.delete_term(Term::from_field_text(
                self.schema_fields.id,
                &item.id.to_lowercase(),
            ));
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
                .with_context(|| "Failed to retrieve user document.".to_string())?;

            let mut id: Option<String> = None;
            let mut user_handle: Option<String> = None;
            let mut content: Option<String> = None;
            let mut timestamp: Option<OffsetDateTime> = None;
            for field_value in doc {
                if field_value.field == self.schema_fields.id {
                    if let Value::Str(field_value_content) = field_value.value {
                        id.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.user_handle {
                    if let Value::Str(field_value_content) = field_value.value {
                        user_handle.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.content {
                    if let Value::Str(field_value_content) = field_value.value {
                        content.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.timestamp {
                    if let Value::Date(field_value_content) = field_value.value {
                        timestamp.replace(field_value_content.into_utc());
                    }
                }
            }

            match (id, content, timestamp, user_handle) {
                (Some(id), Some(content), Some(timestamp), user_handle)
                    if !id.is_empty() && !content.is_empty() =>
                {
                    found_docs.push(SearchItem {
                        id,
                        content,
                        timestamp,
                        user_handle,
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
    };
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn can_index_and_retrieve_items() -> anyhow::Result<()> {
        let index = SearchIndex::open(open_index)?;
        assert_eq!(index.get("some-id")?, None);

        let items = vec![
            MockSearchItemBuilder::new(
                "dev@secutils.dev",
                "content",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .build(),
            MockSearchItemBuilder::new(
                "prod@secutils.dev",
                "prod-content",
                // January 1, 2010 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .set_user_handle("some-handle")
            .build(),
        ];
        for item in items {
            index.upsert(item)?;
        }

        assert_debug_snapshot!(index.get("dev@secutils.dev")?, @r###"
        Some(
            SearchItem {
                id: "dev@secutils.dev",
                content: "content",
                user_handle: None,
                timestamp: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(index.get("prod@secutils.dev")?, @r###"
        Some(
            SearchItem {
                id: "prod@secutils.dev",
                content: "prod-content",
                user_handle: Some(
                    "some-handle",
                ),
                timestamp: 2010-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);
        assert_debug_snapshot!(index.get("user@secutils.dev")?, @"None");
        assert_eq!(index.get("unknown@secutils.dev")?, None);

        Ok(())
    }

    #[test]
    fn ignores_id_case() -> anyhow::Result<()> {
        let item = MockSearchItemBuilder::new(
            "DeV@secutils.dev",
            "DeV-content",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_user_handle("HaNdLe")
        .build();
        let index = SearchIndex::open(open_index)?;
        index.upsert(&item)?;

        assert_eq!(index.get("dev@secutils.dev")?, Some(item.clone()));
        assert_eq!(index.get("DEV@secutils.dev")?, Some(item.clone()));
        assert_eq!(index.get("DeV@secutils.dev")?, Some(item));

        Ok(())
    }

    #[test]
    fn can_update_item() -> anyhow::Result<()> {
        let index = SearchIndex::open(open_index)?;

        index.upsert(
            MockSearchItemBuilder::new(
                "dev@secutils.dev",
                "dev-content",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .build(),
        )?;
        assert_debug_snapshot!(index.get("dev@secutils.dev")?, @r###"
        Some(
            SearchItem {
                id: "dev@secutils.dev",
                content: "dev-content",
                user_handle: None,
                timestamp: 2000-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);

        index.upsert(
            MockSearchItemBuilder::new(
                "DEV@secutils.dev",
                "DEV-content",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .set_user_handle("Some-Handle")
            .build(),
        )?;
        assert_debug_snapshot!(index.get("dev@secutils.dev")?, @r###"
        Some(
            SearchItem {
                id: "DEV@secutils.dev",
                content: "DEV-content",
                user_handle: Some(
                    "Some-Handle",
                ),
                timestamp: 2010-01-01 10:00:00.0 +00:00:00,
            },
        )
        "###);

        assert_eq!(
            index.get("dev@secutils.dev")?,
            index.get("DEV@secutils.dev")?
        );

        Ok(())
    }

    #[test]
    fn can_search() -> anyhow::Result<()> {
        let index = SearchIndex::open(open_index)?;
        let item_dev = MockSearchItemBuilder::new(
            "dev@secutils.dev",
            "dev-content",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_user_handle("Some-Handle")
        .build();
        let item_prod = MockSearchItemBuilder::new(
            "prod@secutils.dev",
            "prod-content",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_user_handle("OTHER-handle")
        .build();

        index.upsert(&item_dev)?;
        index.upsert(&item_prod)?;

        let mut all_items = index.search(SearchFilter::default())?;
        all_items.sort_by(|item_a, item_b| item_a.id.cmp(&item_b.id));
        assert_eq!(all_items, vec![item_dev.clone(), item_prod.clone()]);

        assert_eq!(
            index.search(SearchFilter::default().with_user_handle("some-handle"))?,
            vec![item_dev]
        );
        assert_eq!(
            index.search(SearchFilter::default().with_user_handle("other-handle"))?,
            vec![item_prod]
        );

        assert_eq!(
            index.search(SearchFilter::default().with_user_handle("unknown-handle"))?,
            vec![]
        );

        Ok(())
    }
}
