use super::search_index_schema_fields::SearchIndexSchemaFields;
use crate::{
    directories::Directories,
    search::{SearchFilter, SearchItem},
    users::UserId,
};
use anyhow::{bail, Context};
use std::{collections::HashMap, path::Path, thread, time::Duration};
use tantivy::{
    collector::TopDocs,
    directory::{error::LockError, MmapDirectory},
    error::TantivyError,
    query::{BooleanQuery, Occur, Query, QueryParser, TermQuery},
    schema::*,
    tokenizer::{LowerCaser, NgramTokenizer, RawTokenizer, TextAnalyzer},
    Index, IndexReader, IndexWriter, ReloadPolicy,
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
        *entity.user_id.unwrap_or_else(UserId::empty),
    );

    doc.add_text(schema_fields.label, &entity.label);
    doc.add_text(schema_fields.label_ngram, &entity.label.to_lowercase());
    doc.add_text(schema_fields.category, &entity.category);
    if let Some(ref keywords) = entity.keywords {
        doc.add_text(schema_fields.keywords, keywords);
        for keyword in keywords.split(' ') {
            doc.add_text(schema_fields.keywords_ngram, keyword.to_lowercase());
        }
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
    /// Opens `Search` index at the specified path.
    pub fn open_path<P: AsRef<Path>>(index_path: P) -> anyhow::Result<Self> {
        Self::open(|schema| {
            Directories::ensure_dir_exists(&index_path)?;

            let index_directory = MmapDirectory::open(&index_path)?;

            let index = if Index::exists(&index_directory)? {
                Index::open_in_dir(&index_path)?
            } else {
                log::warn!(
                    "Search index data folder doesn't exist and will be created: {:?}.",
                    index_path.as_ref()
                );
                Index::create_in_dir(&index_path, schema)?
            };

            Ok(index)
        })
    }

    /// Opens `Search` index using the specified initializer.
    pub fn open<I: FnOnce(Schema) -> anyhow::Result<Index>>(
        initializer: I,
    ) -> anyhow::Result<Self> {
        let (schema_fields, schema) = SearchIndexSchemaFields::build();
        let index = initializer(schema)?;

        let ids_tokenizer = TextAnalyzer::builder(RawTokenizer::default())
            .filter(LowerCaser)
            .build();

        let tokenizers = index.tokenizers();
        tokenizers.register("ngram2_10", NgramTokenizer::prefix_only(2, 10)?);
        tokenizers.register("ids", ids_tokenizer.clone());

        let tokenizers = index.fast_field_tokenizer();
        tokenizers.register("ids", ids_tokenizer);

        let index_reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;

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
        self.execute_query(self.search_filter_into_query(filter)?)
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

        self.commit(&mut index_writer)
    }

    /// Removes search item from the `Search` index.
    pub fn remove(&self, id: u64) -> anyhow::Result<()> {
        let mut index_writer = self.acquire_index_writer()?;
        index_writer.delete_term(Term::from_field_u64(self.schema_fields.id, id));

        self.commit(&mut index_writer)
    }

    fn search_filter_into_query(
        &self,
        search_filter: SearchFilter,
    ) -> anyhow::Result<Box<dyn Query>> {
        let public_query = Box::new(TermQuery::new(
            Term::from_field_i64(self.schema_fields.user_id, *UserId::empty()),
            IndexRecordOption::Basic,
        )) as Box<dyn Query>;

        let user_id_query = if let Some(user_id) = search_filter.user_id {
            Box::new(BooleanQuery::new(vec![
                (Occur::Should, public_query),
                (
                    Occur::Should,
                    Box::new(TermQuery::new(
                        Term::from_field_i64(self.schema_fields.user_id, *user_id),
                        IndexRecordOption::Basic,
                    )) as Box<dyn Query>,
                ),
            ])) as Box<dyn Query>
        } else {
            public_query
        };

        let keywords_query = search_filter
            .query
            .map(|query| {
                QueryParser::for_index(
                    &self.index,
                    vec![
                        self.schema_fields.label_ngram,
                        self.schema_fields.keywords_ngram,
                    ],
                )
                .parse_query(&query.to_lowercase())
            })
            .transpose()?;

        let category_query = search_filter.category.map(|category| {
            Box::new(TermQuery::new(
                Term::from_field_text(self.schema_fields.category, category),
                IndexRecordOption::Basic,
            )) as Box<dyn Query>
        });

        // Return either only public items or public items + items for the specific user.
        if keywords_query.is_some() || category_query.is_some() {
            Ok(Box::new(BooleanQuery::new(
                [Some(user_id_query), keywords_query, category_query]
                    .into_iter()
                    .filter_map(|query| Some((Occur::Must, query?)))
                    .collect(),
            )))
        } else {
            Ok(user_id_query)
        }
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
            let mut keywords: Option<String> = None;
            let mut meta: Option<HashMap<String, String>> = None;
            let mut timestamp: Option<OffsetDateTime> = None;
            for field_value in doc {
                if field_value.field == self.schema_fields.id {
                    if let Value::U64(field_value_content) = field_value.value {
                        id.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.user_id {
                    if let Value::I64(field_value_content) = field_value.value {
                        if field_value_content != *UserId::empty() {
                            user_id.replace(field_value_content.try_into()?);
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
                } else if field_value.field == self.schema_fields.keywords {
                    if let Value::Str(field_value_content) = field_value.value {
                        keywords.replace(field_value_content);
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
                        keywords,
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

    fn commit(&self, index_writer: &mut IndexWriter) -> anyhow::Result<()> {
        Ok(index_writer
            .prepare_commit()
            .and_then(|mut prepared_commit| {
                prepared_commit
                    .set_payload(&OffsetDateTime::now_utc().unix_timestamp_nanos().to_string());
                prepared_commit.commit()?;
                self.index_reader.reload()
            })?)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        search::{SearchFilter, SearchIndex},
        tests::MockSearchItemBuilder,
    };
    use insta::assert_debug_snapshot;
    use tantivy::Index;
    use time::OffsetDateTime;

    #[test]
    fn can_index_and_retrieve_items() -> anyhow::Result<()> {
        let index = SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))?;
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
            .set_user_id(3.try_into()?)
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
                category: "some-category",
                keywords: None,
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
                category: "other-category",
                keywords: Some(
                    "some keywords",
                ),
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
        let index = SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))?;

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
                category: "some-category",
                keywords: None,
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
                category: "other-category",
                keywords: None,
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
        let index = SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))?;
        let item_user_3 = MockSearchItemBuilder::new(
            1,
            "some-label",
            "some-category",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_user_id(3.try_into()?)
        .build();
        let item_user_4 = MockSearchItemBuilder::new(
            2,
            "other-label",
            "other-category",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_user_id(4.try_into()?)
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
            index.search(SearchFilter::default().with_user_id(3.try_into()?))?;
        public_and_user_items.sort_by(|item_a, item_b| item_a.id.cmp(&item_b.id));
        assert_eq!(
            public_and_user_items,
            vec![item_user_3, public_item.clone()]
        );

        let mut public_and_user_items =
            index.search(SearchFilter::default().with_user_id(4.try_into()?))?;
        public_and_user_items.sort_by(|item_a, item_b| item_a.id.cmp(&item_b.id));
        assert_eq!(
            public_and_user_items,
            vec![item_user_4, public_item.clone()]
        );

        assert_eq!(
            index.search(SearchFilter::default().with_user_id(5.try_into()?))?,
            vec![public_item]
        );

        Ok(())
    }

    #[test]
    fn can_remove() -> anyhow::Result<()> {
        let index = SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))?;
        let item_1 = MockSearchItemBuilder::new(
            1,
            "some-label",
            "some-category",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .build();
        let item_2 = MockSearchItemBuilder::new(
            2,
            "other-label",
            "other-category",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build();

        index.upsert(&item_1)?;
        index.upsert(&item_2)?;

        let mut items = index.search(SearchFilter::default())?;
        items.sort_by(|item_a, item_b| item_a.id.cmp(&item_b.id));
        assert_eq!(items, vec![item_1.clone(), item_2.clone()]);

        index.remove(item_1.id)?;
        assert_eq!(index.search(SearchFilter::default())?, vec![item_2.clone()]);

        index.remove(item_2.id)?;
        assert_eq!(index.search(SearchFilter::default())?, vec![]);

        Ok(())
    }

    #[test]
    fn default_filter() -> anyhow::Result<()> {
        let default_filter = SearchFilter::default();
        assert_eq!(
            default_filter,
            SearchFilter {
                user_id: None,
                query: None,
                category: None
            }
        );

        let index = SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))?;
        assert_debug_snapshot!(
            index.search_filter_into_query(default_filter)?,
            @"TermQuery(Term(field=1, type=I64, 0))"
        );

        Ok(())
    }

    #[test]
    fn filter_with_user_id() -> anyhow::Result<()> {
        let filter = SearchFilter::default().with_user_id(1.try_into()?);
        assert_eq!(
            filter,
            SearchFilter {
                user_id: Some(1.try_into()?),
                query: None,
                category: None
            }
        );

        let index = SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))?;
        assert_debug_snapshot!(
             index.search_filter_into_query(filter)?,
            @r###"
        BooleanQuery {
            subqueries: [
                (
                    Should,
                    TermQuery(Term(field=1, type=I64, 0)),
                ),
                (
                    Should,
                    TermQuery(Term(field=1, type=I64, 1)),
                ),
            ],
        }
        "###
        );

        Ok(())
    }

    #[test]
    fn filter_with_query() -> anyhow::Result<()> {
        let filter = SearchFilter::default().with_query("Some-Query");
        assert_eq!(
            filter,
            SearchFilter {
                user_id: None,
                query: Some("Some-Query"),
                category: None
            }
        );

        let index = SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))?;
        assert_debug_snapshot!(
            index.search_filter_into_query(filter)?,
            @r###"
        BooleanQuery {
            subqueries: [
                (
                    Must,
                    TermQuery(Term(field=1, type=I64, 0)),
                ),
                (
                    Must,
                    BooleanQuery {
                        subqueries: [
                            (
                                Should,
                                PhraseQuery {
                                    field: Field(
                                        3,
                                    ),
                                    phrase_terms: [
                                        (
                                            0,
                                            Term(field=3, type=Str, "so"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "som"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-q"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-qu"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-que"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-quer"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-query"),
                                        ),
                                    ],
                                    slop: 0,
                                },
                            ),
                            (
                                Should,
                                PhraseQuery {
                                    field: Field(
                                        5,
                                    ),
                                    phrase_terms: [
                                        (
                                            0,
                                            Term(field=5, type=Str, "so"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "som"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-q"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-qu"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-que"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-quer"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-query"),
                                        ),
                                    ],
                                    slop: 0,
                                },
                            ),
                        ],
                    },
                ),
            ],
        }
        "###
        );

        Ok(())
    }

    #[test]
    fn filter_with_category() -> anyhow::Result<()> {
        let filter = SearchFilter::default().with_category("Some-Category");
        assert_eq!(
            filter,
            SearchFilter {
                user_id: None,
                query: None,
                category: Some("Some-Category")
            }
        );

        let index = SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))?;
        assert_debug_snapshot!(
            index.search_filter_into_query(filter)?,
            @r###"
        BooleanQuery {
            subqueries: [
                (
                    Must,
                    TermQuery(Term(field=1, type=I64, 0)),
                ),
                (
                    Must,
                    TermQuery(Term(field=6, type=Str, "Some-Category")),
                ),
            ],
        }
        "###
        );

        Ok(())
    }

    #[test]
    fn filter_with_user_id_and_query() -> anyhow::Result<()> {
        let filter = SearchFilter::default()
            .with_user_id(1.try_into()?)
            .with_query("Some-Query");
        assert_eq!(
            filter,
            SearchFilter {
                user_id: Some(1.try_into()?),
                query: Some("Some-Query"),
                category: None
            }
        );

        let index = SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))?;
        assert_debug_snapshot!(
            index.search_filter_into_query(filter)?,
            @r###"
        BooleanQuery {
            subqueries: [
                (
                    Must,
                    BooleanQuery {
                        subqueries: [
                            (
                                Should,
                                TermQuery(Term(field=1, type=I64, 0)),
                            ),
                            (
                                Should,
                                TermQuery(Term(field=1, type=I64, 1)),
                            ),
                        ],
                    },
                ),
                (
                    Must,
                    BooleanQuery {
                        subqueries: [
                            (
                                Should,
                                PhraseQuery {
                                    field: Field(
                                        3,
                                    ),
                                    phrase_terms: [
                                        (
                                            0,
                                            Term(field=3, type=Str, "so"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "som"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-q"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-qu"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-que"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-quer"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-query"),
                                        ),
                                    ],
                                    slop: 0,
                                },
                            ),
                            (
                                Should,
                                PhraseQuery {
                                    field: Field(
                                        5,
                                    ),
                                    phrase_terms: [
                                        (
                                            0,
                                            Term(field=5, type=Str, "so"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "som"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-q"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-qu"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-que"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-quer"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-query"),
                                        ),
                                    ],
                                    slop: 0,
                                },
                            ),
                        ],
                    },
                ),
            ],
        }
        "###
        );

        Ok(())
    }

    #[test]
    fn filter_with_category_and_query() -> anyhow::Result<()> {
        let filter = SearchFilter::default()
            .with_category("Some-Category")
            .with_query("Some-Query");
        assert_eq!(
            filter,
            SearchFilter {
                user_id: None,
                query: Some("Some-Query"),
                category: Some("Some-Category")
            }
        );

        let index = SearchIndex::open(|schema| Ok(Index::create_in_ram(schema)))?;
        assert_debug_snapshot!(
            index.search_filter_into_query(filter)?,
            @r###"
        BooleanQuery {
            subqueries: [
                (
                    Must,
                    TermQuery(Term(field=1, type=I64, 0)),
                ),
                (
                    Must,
                    BooleanQuery {
                        subqueries: [
                            (
                                Should,
                                PhraseQuery {
                                    field: Field(
                                        3,
                                    ),
                                    phrase_terms: [
                                        (
                                            0,
                                            Term(field=3, type=Str, "so"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "som"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-q"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-qu"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-que"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-quer"),
                                        ),
                                        (
                                            0,
                                            Term(field=3, type=Str, "some-query"),
                                        ),
                                    ],
                                    slop: 0,
                                },
                            ),
                            (
                                Should,
                                PhraseQuery {
                                    field: Field(
                                        5,
                                    ),
                                    phrase_terms: [
                                        (
                                            0,
                                            Term(field=5, type=Str, "so"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "som"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-q"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-qu"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-que"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-quer"),
                                        ),
                                        (
                                            0,
                                            Term(field=5, type=Str, "some-query"),
                                        ),
                                    ],
                                    slop: 0,
                                },
                            ),
                        ],
                    },
                ),
                (
                    Must,
                    TermQuery(Term(field=6, type=Str, "Some-Category")),
                ),
            ],
        }
        "###
        );

        Ok(())
    }
}
