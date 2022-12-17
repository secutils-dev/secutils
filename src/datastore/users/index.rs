use crate::{
    datastore::{commit_index, UsersSchemaFields, UsersSearchFilter},
    users::{User, UserProfile},
};
use anyhow::{bail, Context};
use std::collections::HashSet;
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
    entity: &User,
    schema_fields: &UsersSchemaFields,
) -> anyhow::Result<Document> {
    let mut doc = Document::default();

    doc.add_text(schema_fields.email, &entity.email);
    doc.add_text(schema_fields.handle, &entity.handle);
    doc.add_text(schema_fields.password_hash, &entity.password_hash);
    doc.add_date(
        schema_fields.created,
        tantivy::DateTime::from_utc(entity.created),
    );

    for role in entity.roles.iter() {
        doc.add_text(schema_fields.roles, role.to_lowercase());
    }

    if let Some(ref profile) = entity.profile {
        doc.add_bytes(
            schema_fields.profile,
            serde_json::ser::to_vec(profile).with_context(|| {
                format!(
                    "Failed to serialize profile for user {}: {:?}",
                    entity.email, profile
                )
            })?,
        );
    }

    if let Some(ref activation_code) = entity.activation_code {
        doc.add_text(schema_fields.activation_code, activation_code);
    }

    Ok(doc)
}

#[derive(Clone)]
pub struct UsersIndex {
    schema_fields: UsersSchemaFields,
    index: Index,
    index_reader: IndexReader,
}

impl UsersIndex {
    /// Opens `Users` index.
    pub fn open<I: FnOnce(Schema) -> anyhow::Result<(Index, IndexReader)>>(
        initializer: I,
    ) -> anyhow::Result<Self> {
        let (schema_fields, schema) = UsersSchemaFields::build();
        let (index, index_reader) = initializer(schema)?;

        Ok(Self {
            schema_fields,
            index,
            index_reader,
        })
    }

    /// Retrieves user from the `Users` index using user email.
    pub fn get<T: AsRef<str>>(&self, email: T) -> anyhow::Result<Option<User>> {
        let email_query: Box<dyn Query> = Box::new(TermQuery::new(
            Term::from_field_text(self.schema_fields.email, &email.as_ref().to_lowercase()),
            IndexRecordOption::Basic,
        )) as Box<dyn Query>;

        self.execute_query(email_query).and_then(|mut entities| {
            if entities.is_empty() {
                return Ok(None);
            }

            if entities.len() > 1 {
                bail!(
                    "Founds {} users for the same email {}.",
                    entities.len().to_string(),
                    email.as_ref()
                );
            }

            Ok(Some(entities.remove(0)))
        })
    }

    /// Retrieves user from the `Users` index using user handle.
    pub fn get_by_handle<T: AsRef<str>>(&self, handle: T) -> anyhow::Result<Option<User>> {
        let handle_query: Box<dyn Query> = Box::new(TermQuery::new(
            Term::from_field_text(self.schema_fields.handle, &handle.as_ref().to_lowercase()),
            IndexRecordOption::Basic,
        )) as Box<dyn Query>;

        self.execute_query(handle_query).and_then(|mut entities| {
            if entities.is_empty() {
                return Ok(None);
            }

            if entities.len() > 1 {
                bail!(
                    "Founds {} users for the same handle {}.",
                    entities.len().to_string(),
                    handle.as_ref()
                );
            }

            Ok(Some(entities.remove(0)))
        })
    }

    /// Search for users using the specified filter.
    pub fn search(&self, filter: UsersSearchFilter) -> anyhow::Result<Vec<User>> {
        self.execute_query(filter.into_query(&self.schema_fields))
    }

    /// Inserts or updates user in the `Users` index.
    pub fn upsert<U: AsRef<User>>(&self, user: U) -> anyhow::Result<()> {
        let user = user.as_ref();
        let existing_user = self.get(&user.email)?;

        let mut index_writer = self.acquire_index_writer()?;
        if existing_user.is_some() {
            index_writer.delete_term(Term::from_field_text(
                self.schema_fields.email,
                &user.email.to_lowercase(),
            ));
        }

        index_writer.add_document(entity_to_document(user, &self.schema_fields)?)?;

        commit_index(&mut index_writer, &self.index_reader)
    }

    /// Removes user with the specified email from the `Users` index.
    pub fn remove<T: AsRef<str>>(&self, email: T) -> anyhow::Result<Option<User>> {
        let email_lower_case = email.as_ref().to_lowercase();

        let existing_user = self.get(&email_lower_case)?;
        if existing_user.is_none() {
            return Ok(None);
        }

        let mut index_writer = self.acquire_index_writer()?;
        index_writer.delete_term(Term::from_field_text(
            self.schema_fields.email,
            &email_lower_case,
        ));

        commit_index(&mut index_writer, &self.index_reader).map(|_| existing_user)
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

    fn execute_query(&self, query: impl Query) -> anyhow::Result<Vec<User>> {
        let searcher = self.index_reader.searcher();

        let collector = TopDocs::with_limit(10000);
        let top_docs = searcher.search(&query, &collector)?;

        let mut found_docs = Vec::with_capacity(top_docs.len());
        for (_, doc_address) in top_docs.into_iter() {
            let doc = searcher
                .doc(doc_address)
                .with_context(|| "Failed to retrieve user document.".to_string())?;

            let mut email: Option<String> = None;
            let mut handle: Option<String> = None;
            let mut password_hash: Option<String> = None;
            let mut created: Option<OffsetDateTime> = None;
            let mut roles: Option<HashSet<String>> = None;
            let mut profile: Option<UserProfile> = None;
            let mut activation_code: Option<String> = None;
            for field_value in doc {
                if field_value.field == self.schema_fields.email {
                    if let Value::Str(field_value_content) = field_value.value {
                        email.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.handle {
                    if let Value::Str(field_value_content) = field_value.value {
                        handle.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.password_hash {
                    if let Value::Str(field_value_content) = field_value.value {
                        password_hash.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.created {
                    if let Value::Date(field_value_content) = field_value.value {
                        created.replace(field_value_content.into_utc());
                    }
                } else if field_value.field == self.schema_fields.activation_code {
                    if let Value::Str(field_value_content) = field_value.value {
                        activation_code.replace(field_value_content);
                    }
                } else if field_value.field == self.schema_fields.profile {
                    if let Value::Bytes(field_value_content) = field_value.value {
                        profile.replace(
                            serde_json::from_slice::<UserProfile>(&field_value_content)
                                .with_context(|| "Cannot deserialize user profile.".to_string())?,
                        );
                    }
                } else if field_value.field == self.schema_fields.roles {
                    if let Value::Str(field_value_content) = field_value.value {
                        match roles.as_mut() {
                            Some(roles) => {
                                roles.insert(field_value_content);
                            }
                            None => {
                                roles.replace(HashSet::from_iter([field_value_content]));
                            }
                        }
                    }
                }
            }

            match (email, handle, password_hash, created) {
                (Some(email), Some(handle), Some(password_hash), Some(created))
                    if !email.is_empty() && !handle.is_empty() && !password_hash.is_empty() =>
                {
                    found_docs.push(User {
                        email,
                        handle,
                        password_hash,
                        created,
                        roles: roles.unwrap_or_else(|| HashSet::with_capacity(0)),
                        profile,
                        activation_code,
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
        datastore::{UsersIndex, UsersSearchFilter},
        tests::{open_index, MockUserBuilder},
        users::UserProfile,
    };
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn can_index_and_retrieve_users() -> anyhow::Result<()> {
        let index = UsersIndex::open(open_index)?;
        assert_eq!(index.get("dev@secutils.dev")?, None);

        let users = vec![
            MockUserBuilder::new(
                "dev@secutils.dev",
                "dev-handle",
                "hash",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .build(),
            MockUserBuilder::new(
                "prod@secutils.dev",
                "prod-handle",
                "hash_prod",
                // January 1, 2010 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .set_activation_code("some-code")
            .add_role("admin")
            .build(),
            MockUserBuilder::new(
                "user@secutils.dev",
                "handle",
                "hash",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .set_activation_code("some-user-code")
            .set_profile(UserProfile::default())
            .add_role("Power-User")
            .build(),
        ];
        for user in users {
            index.upsert(&user)?;
        }

        assert_debug_snapshot!(index.get("dev@secutils.dev")?, @r###"
        Some(
            User {
                email: "dev@secutils.dev",
                handle: "dev-handle",
                password_hash: "hash",
                roles: {},
                created: 2000-01-01 10:00:00.0 +00:00:00,
                profile: None,
                activation_code: None,
            },
        )
        "###);
        assert_debug_snapshot!(index.get("prod@secutils.dev")?, @r###"
        Some(
            User {
                email: "prod@secutils.dev",
                handle: "prod-handle",
                password_hash: "hash_prod",
                roles: {
                    "admin",
                },
                created: 2010-01-01 10:00:00.0 +00:00:00,
                profile: None,
                activation_code: Some(
                    "some-code",
                ),
            },
        )
        "###);
        assert_debug_snapshot!(index.get("user@secutils.dev")?, @r###"
        Some(
            User {
                email: "user@secutils.dev",
                handle: "handle",
                password_hash: "hash",
                roles: {
                    "power-user",
                },
                created: 2000-01-01 10:00:00.0 +00:00:00,
                profile: Some(
                    UserProfile {
                        data: None,
                    },
                ),
                activation_code: Some(
                    "some-user-code",
                ),
            },
        )
        "###);
        assert_eq!(index.get("unknown@secutils.dev")?, None);

        Ok(())
    }

    #[test]
    fn ignores_email_case() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            "DeV@secutils.dev",
            "DeV-handle",
            "hash",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .add_role("user")
        .add_role("Power-User")
        .build();
        let index = UsersIndex::open(open_index)?;
        index.upsert(&user)?;

        assert_eq!(index.get("dev@secutils.dev")?, Some(user.clone()));
        assert_eq!(index.get("DEV@secutils.dev")?, Some(user.clone()));
        assert_eq!(index.get("DeV@secutils.dev")?, Some(user));

        Ok(())
    }

    #[test]
    fn ignores_handle_case() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            "DeV@secutils.dev",
            "DeV-handle",
            "hash",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .add_role("user")
        .add_role("Power-User")
        .build();
        let index = UsersIndex::open(open_index)?;
        index.upsert(&user)?;

        assert_eq!(index.get_by_handle("dev-handle")?, Some(user.clone()));
        assert_eq!(index.get_by_handle("DEV-handle")?, Some(user.clone()));
        assert_eq!(index.get_by_handle("DeV-handle")?, Some(user));

        Ok(())
    }

    #[test]
    fn can_update_user() -> anyhow::Result<()> {
        let index = UsersIndex::open(open_index)?;

        index.upsert(
            &MockUserBuilder::new(
                "dev@secutils.dev",
                "dev-handle",
                "hash",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(946720800)?,
            )
            .build(),
        )?;
        assert_debug_snapshot!(index.get("dev@secutils.dev")?, @r###"
        Some(
            User {
                email: "dev@secutils.dev",
                handle: "dev-handle",
                password_hash: "hash",
                roles: {},
                created: 2000-01-01 10:00:00.0 +00:00:00,
                profile: None,
                activation_code: None,
            },
        )
        "###);

        index.upsert(
            &MockUserBuilder::new(
                "DEV@secutils.dev",
                "DEV-handle",
                "new-hash",
                // January 1, 2000 11:00:00
                OffsetDateTime::from_unix_timestamp(1262340000)?,
            )
            .set_activation_code("some-code")
            .set_profile(UserProfile::default())
            .add_role("admin")
            .build(),
        )?;
        assert_debug_snapshot!(index.get("dev@secutils.dev")?, @r###"
        Some(
            User {
                email: "DEV@secutils.dev",
                handle: "DEV-handle",
                password_hash: "new-hash",
                roles: {
                    "admin",
                },
                created: 2010-01-01 10:00:00.0 +00:00:00,
                profile: Some(
                    UserProfile {
                        data: None,
                    },
                ),
                activation_code: Some(
                    "some-code",
                ),
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
    fn can_remove_user() -> anyhow::Result<()> {
        let index = UsersIndex::open(open_index)?;
        assert_eq!(index.get("dev@secutils.dev")?, None);
        assert_eq!(index.get("prod@secutils.dev")?, None);

        let user_dev = MockUserBuilder::new(
            "dev@secutils.dev",
            "dev-handle",
            "hash",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .build();
        let user_prod = MockUserBuilder::new(
            "prod@secutils.dev",
            "prod-handle",
            "hash_prod",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_activation_code("some-code")
        .build();

        index.upsert(&user_dev)?;
        index.upsert(&user_prod)?;

        assert_eq!(index.get("dev@secutils.dev")?, Some(user_dev.clone()));
        assert_eq!(index.get("prod@secutils.dev")?, Some(user_prod.clone()));

        assert_eq!(index.remove("dev@secutils.dev")?, Some(user_dev));
        assert_eq!(index.get("dev@secutils.dev")?, None);
        assert_eq!(index.remove("dev@secutils.dev")?, None);
        assert_eq!(index.get("prod@secutils.dev")?, Some(user_prod.clone()));

        assert_eq!(index.remove("prod@secutils.dev")?, Some(user_prod));
        assert_eq!(index.get("prod@secutils.dev")?, None);
        assert_eq!(index.remove("prod@secutils.dev")?, None);

        Ok(())
    }

    #[test]
    fn can_search_users() -> anyhow::Result<()> {
        let index = UsersIndex::open(open_index)?;
        let user_dev = MockUserBuilder::new(
            "dev@secutils.dev",
            "dev-handle",
            "hash",
            // January 1, 2000 11:00:00
            OffsetDateTime::from_unix_timestamp(946720800)?,
        )
        .set_activation_code("some-code")
        .build();
        let user_prod = MockUserBuilder::new(
            "prod@secutils.dev",
            "prod-handle",
            "hash_prod",
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .set_activation_code("OTHER-code")
        .build();

        index.upsert(&user_dev)?;
        index.upsert(&user_prod)?;

        let mut all_users = index.search(UsersSearchFilter::default())?;
        all_users.sort_by(|user_a, user_b| user_a.email.cmp(&user_b.email));
        assert_eq!(all_users, vec![user_dev.clone(), user_prod.clone()]);

        assert_eq!(
            index.search(UsersSearchFilter::default().with_activation_code("some-code"))?,
            vec![user_dev.clone()]
        );
        assert_eq!(
            index.search(UsersSearchFilter::default().with_activation_code("SOME-code"))?,
            vec![user_dev.clone()]
        );

        assert_eq!(
            index.search(UsersSearchFilter::default().with_activation_code("other-code"))?,
            vec![user_prod.clone()]
        );
        assert_eq!(
            index.search(UsersSearchFilter::default().with_activation_code("OTHER-code"))?,
            vec![user_prod.clone()]
        );

        assert_eq!(
            index.search(UsersSearchFilter::default().with_activation_code("unknown-code"))?,
            vec![]
        );

        Ok(())
    }
}
