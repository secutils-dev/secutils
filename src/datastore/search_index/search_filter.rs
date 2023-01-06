use crate::datastore::SearchIndexSchemaFields;
use tantivy::{
    query::{AllQuery, Query, TermQuery},
    schema::IndexRecordOption,
    Term,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchFilter<'a> {
    user_handle: Option<&'a str>,
}

impl<'a> SearchFilter<'a> {
    pub fn with_user_handle(self, handle: &'a str) -> Self {
        Self {
            user_handle: Some(handle),
        }
    }

    pub fn into_query(self, schema_fields: &SearchIndexSchemaFields) -> Box<dyn Query> {
        let handle_query = self.user_handle.map(|user_handle| {
            Box::new(TermQuery::new(
                Term::from_field_text(schema_fields.user_handle, &user_handle.to_lowercase()),
                IndexRecordOption::Basic,
            )) as Box<dyn Query>
        });

        match handle_query {
            Some(handle_query) => handle_query,
            None => Box::new(AllQuery {}),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::datastore::{SearchFilter, SearchIndexSchemaFields};
    use insta::assert_debug_snapshot;

    #[test]
    fn default_filter() {
        let default_filter = SearchFilter::default();
        assert_eq!(default_filter, SearchFilter { user_handle: None });

        let (schema_fields, _) = SearchIndexSchemaFields::build();
        assert_debug_snapshot!(
            default_filter.into_query(&schema_fields),
            @"AllQuery"
        );
    }

    #[test]
    fn filter_with_user_handle() {
        let filter = SearchFilter::default().with_user_handle("Some-User");
        assert_eq!(
            filter,
            SearchFilter {
                user_handle: Some("Some-User"),
            }
        );

        let (schema_fields, _) = SearchIndexSchemaFields::build();
        assert_debug_snapshot!(
            filter.into_query(&schema_fields),
            @r###"TermQuery(Term(type=Str, field=1, "some-user"))"###
        );
    }
}
