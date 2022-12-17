use crate::datastore::UsersSchemaFields;
use tantivy::{
    query::{AllQuery, Query, TermQuery},
    schema::IndexRecordOption,
    Term,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsersSearchFilter<'a> {
    activation_code: Option<&'a str>,
}

impl<'a> UsersSearchFilter<'a> {
    pub fn with_activation_code(self, activation_code: &'a str) -> Self {
        Self {
            activation_code: Some(activation_code),
        }
    }

    pub fn into_query(self, schema_fields: &UsersSchemaFields) -> Box<dyn Query> {
        let activation_code_query = self.activation_code.map(|activation_code| {
            Box::new(TermQuery::new(
                Term::from_field_text(
                    schema_fields.activation_code,
                    &activation_code.to_lowercase(),
                ),
                IndexRecordOption::Basic,
            )) as Box<dyn Query>
        });

        match activation_code_query {
            Some(activation_code_query) => activation_code_query,
            None => Box::new(AllQuery {}),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::datastore::{UsersSchemaFields, UsersSearchFilter};
    use insta::assert_debug_snapshot;

    #[test]
    fn default_filter() {
        let default_filter = UsersSearchFilter::default();
        assert_eq!(
            default_filter,
            UsersSearchFilter {
                activation_code: None,
            }
        );

        let (schema_fields, _) = UsersSchemaFields::build();
        assert_debug_snapshot!(
            default_filter.into_query(&schema_fields),
            @"AllQuery"
        );
    }

    #[test]
    fn filter_with_activation_code() {
        let filter = UsersSearchFilter::default().with_activation_code("Some-Code");
        assert_eq!(
            filter,
            UsersSearchFilter {
                activation_code: Some("Some-Code"),
            }
        );

        let (schema_fields, _) = UsersSchemaFields::build();
        assert_debug_snapshot!(
            filter.into_query(&schema_fields),
            @r###"TermQuery(Term(type=Str, field=6, "some-code"))"###
        );
    }
}
