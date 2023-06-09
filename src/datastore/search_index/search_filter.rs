use crate::{datastore::SearchIndexSchemaFields, users::UserId};
use tantivy::{
    query::{BooleanQuery, Occur, Query, QueryParser, TermQuery},
    schema::IndexRecordOption,
    Index, Term,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchFilter<'q, 'c> {
    user_id: Option<UserId>,
    query: Option<&'q str>,
    category: Option<&'c str>,
}

impl<'q, 'c> SearchFilter<'q, 'c> {
    pub fn with_user_id(self, user_id: UserId) -> Self {
        Self {
            user_id: Some(user_id),
            ..self
        }
    }

    pub fn with_query(self, query: &'q str) -> Self {
        Self {
            query: Some(query),
            ..self
        }
    }

    pub fn with_category(self, category: &'c str) -> Self {
        Self {
            category: Some(category),
            ..self
        }
    }

    pub fn into_query(
        self,
        index: &Index,
        schema_fields: &SearchIndexSchemaFields,
    ) -> anyhow::Result<Box<dyn Query>> {
        let public_query = Box::new(TermQuery::new(
            Term::from_field_i64(schema_fields.user_id, UserId::empty().0),
            IndexRecordOption::Basic,
        )) as Box<dyn Query>;

        let user_id_query = if let Some(user_id) = self.user_id {
            Box::new(BooleanQuery::new(vec![
                (Occur::Should, public_query),
                (
                    Occur::Should,
                    Box::new(TermQuery::new(
                        Term::from_field_i64(schema_fields.user_id, user_id.0),
                        IndexRecordOption::Basic,
                    )) as Box<dyn Query>,
                ),
            ])) as Box<dyn Query>
        } else {
            public_query
        };

        let keywords_query = self
            .query
            .map(|query| {
                QueryParser::for_index(
                    index,
                    vec![schema_fields.label_ngram, schema_fields.keywords_ngram],
                )
                .parse_query(&query.to_lowercase())
            })
            .transpose()?;

        let category_query = self.category.map(|category| {
            Box::new(TermQuery::new(
                Term::from_field_text(schema_fields.category, category),
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
}

#[cfg(test)]
mod tests {
    use crate::{
        datastore::{SearchFilter, SearchIndexSchemaFields},
        tests::open_index,
        users::UserId,
    };
    use insta::assert_debug_snapshot;

    #[test]
    fn default_filter() -> anyhow::Result<()> {
        let (index, _) = open_index(SearchIndexSchemaFields::build().1)?;
        let default_filter = SearchFilter::default();
        assert_eq!(
            default_filter,
            SearchFilter {
                user_id: None,
                query: None,
                category: None
            }
        );

        let (schema_fields, _) = SearchIndexSchemaFields::build();
        assert_debug_snapshot!(
            default_filter.into_query(&index, &schema_fields),
            @r###"
        Ok(
            TermQuery(Term(field=1, type=I64, -1)),
        )
        "###
        );

        Ok(())
    }

    #[test]
    fn filter_with_user_id() -> anyhow::Result<()> {
        let (index, _) = open_index(SearchIndexSchemaFields::build().1)?;
        let filter = SearchFilter::default().with_user_id(UserId(1));
        assert_eq!(
            filter,
            SearchFilter {
                user_id: Some(UserId(1)),
                query: None,
                category: None
            }
        );

        let (schema_fields, _) = SearchIndexSchemaFields::build();
        assert_debug_snapshot!(
            filter.into_query(&index, &schema_fields),
            @r###"
        Ok(
            BooleanQuery {
                subqueries: [
                    (
                        Should,
                        TermQuery(Term(field=1, type=I64, -1)),
                    ),
                    (
                        Should,
                        TermQuery(Term(field=1, type=I64, 1)),
                    ),
                ],
            },
        )
        "###
        );

        Ok(())
    }

    #[test]
    fn filter_with_query() -> anyhow::Result<()> {
        let (index, _) = open_index(SearchIndexSchemaFields::build().1)?;
        let filter = SearchFilter::default().with_query("Some-Query");
        assert_eq!(
            filter,
            SearchFilter {
                user_id: None,
                query: Some("Some-Query"),
                category: None
            }
        );

        let (schema_fields, _) = SearchIndexSchemaFields::build();
        assert_debug_snapshot!(
            filter.into_query(&index, &schema_fields),
            @r###"
        Ok(
            BooleanQuery {
                subqueries: [
                    (
                        Must,
                        TermQuery(Term(field=1, type=I64, -1)),
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
            },
        )
        "###
        );

        Ok(())
    }

    #[test]
    fn filter_with_category() -> anyhow::Result<()> {
        let (index, _) = open_index(SearchIndexSchemaFields::build().1)?;
        let filter = SearchFilter::default().with_category("Some-Category");
        assert_eq!(
            filter,
            SearchFilter {
                user_id: None,
                query: None,
                category: Some("Some-Category")
            }
        );

        let (schema_fields, _) = SearchIndexSchemaFields::build();
        assert_debug_snapshot!(
            filter.into_query(&index, &schema_fields),
            @r###"
        Ok(
            BooleanQuery {
                subqueries: [
                    (
                        Must,
                        TermQuery(Term(field=1, type=I64, -1)),
                    ),
                    (
                        Must,
                        TermQuery(Term(field=6, type=Str, "Some-Category")),
                    ),
                ],
            },
        )
        "###
        );

        Ok(())
    }

    #[test]
    fn filter_with_user_id_and_query() -> anyhow::Result<()> {
        let (index, _) = open_index(SearchIndexSchemaFields::build().1)?;
        let filter = SearchFilter::default()
            .with_user_id(UserId(1))
            .with_query("Some-Query");
        assert_eq!(
            filter,
            SearchFilter {
                user_id: Some(UserId(1)),
                query: Some("Some-Query"),
                category: None
            }
        );

        let (schema_fields, _) = SearchIndexSchemaFields::build();
        assert_debug_snapshot!(
            filter.into_query(&index, &schema_fields),
            @r###"
        Ok(
            BooleanQuery {
                subqueries: [
                    (
                        Must,
                        BooleanQuery {
                            subqueries: [
                                (
                                    Should,
                                    TermQuery(Term(field=1, type=I64, -1)),
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
            },
        )
        "###
        );

        Ok(())
    }

    #[test]
    fn filter_with_category_and_query() -> anyhow::Result<()> {
        let (index, _) = open_index(SearchIndexSchemaFields::build().1)?;
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

        let (schema_fields, _) = SearchIndexSchemaFields::build();
        assert_debug_snapshot!(
            filter.into_query(&index, &schema_fields),
            @r###"
        Ok(
            BooleanQuery {
                subqueries: [
                    (
                        Must,
                        TermQuery(Term(field=1, type=I64, -1)),
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
            },
        )
        "###
        );

        Ok(())
    }
}
