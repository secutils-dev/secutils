use crate::{datastore::SearchIndexSchemaFields, users::UserId};
use tantivy::{
    query::{BooleanQuery, Occur, Query, QueryParser, TermQuery},
    schema::IndexRecordOption,
    Index, Term,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchFilter<'q> {
    user_id: Option<UserId>,
    query: Option<&'q str>,
}

impl<'q> SearchFilter<'q> {
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

        let keywords_query: Option<Box<dyn Query>> = if let Some(query) = self.query {
            Some(
                QueryParser::for_index(index, vec![schema_fields.label, schema_fields.keywords])
                    .parse_query(query)?,
            )
        } else {
            None
        };

        // Return either only public items or public items + items for the specific user.
        Ok(match keywords_query {
            Some(keywords_query) => Box::new(BooleanQuery::new(vec![
                (Occur::Must, user_id_query),
                (Occur::Must, keywords_query),
            ])),
            None => user_id_query,
        })
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
                query: None
            }
        );

        let (schema_fields, _) = SearchIndexSchemaFields::build();
        assert_debug_snapshot!(
            default_filter.into_query(&index, &schema_fields),
            @r###"
        Ok(
            TermQuery(Term(type=I64, field=1, -1)),
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
                query: None
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
                        TermQuery(Term(type=I64, field=1, -1)),
                    ),
                    (
                        Should,
                        TermQuery(Term(type=I64, field=1, 1)),
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
                query: Some("Some-Query")
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
                        TermQuery(Term(type=I64, field=1, -1)),
                    ),
                    (
                        Must,
                        BooleanQuery {
                            subqueries: [
                                (
                                    Should,
                                    PhraseQuery {
                                        field: Field(
                                            2,
                                        ),
                                        phrase_terms: [
                                            (
                                                0,
                                                Term(type=Str, field=2, "some"),
                                            ),
                                            (
                                                1,
                                                Term(type=Str, field=2, "query"),
                                            ),
                                        ],
                                        slop: 0,
                                    },
                                ),
                                (
                                    Should,
                                    PhraseQuery {
                                        field: Field(
                                            3,
                                        ),
                                        phrase_terms: [
                                            (
                                                0,
                                                Term(type=Str, field=3, "some"),
                                            ),
                                            (
                                                1,
                                                Term(type=Str, field=3, "query"),
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
    fn filter_with_user_id_and_query() -> anyhow::Result<()> {
        let (index, _) = open_index(SearchIndexSchemaFields::build().1)?;
        let filter = SearchFilter::default()
            .with_user_id(UserId(1))
            .with_query("Some-Query");
        assert_eq!(
            filter,
            SearchFilter {
                user_id: Some(UserId(1)),
                query: Some("Some-Query")
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
                                    TermQuery(Term(type=I64, field=1, -1)),
                                ),
                                (
                                    Should,
                                    TermQuery(Term(type=I64, field=1, 1)),
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
                                            2,
                                        ),
                                        phrase_terms: [
                                            (
                                                0,
                                                Term(type=Str, field=2, "some"),
                                            ),
                                            (
                                                1,
                                                Term(type=Str, field=2, "query"),
                                            ),
                                        ],
                                        slop: 0,
                                    },
                                ),
                                (
                                    Should,
                                    PhraseQuery {
                                        field: Field(
                                            3,
                                        ),
                                        phrase_terms: [
                                            (
                                                0,
                                                Term(type=Str, field=3, "some"),
                                            ),
                                            (
                                                1,
                                                Term(type=Str, field=3, "query"),
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
}
