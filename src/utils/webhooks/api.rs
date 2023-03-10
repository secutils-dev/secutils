use crate::{
    api::Api,
    datastore::PrimaryDb,
    users::{PublicUserDataNamespace, UserData, UserId},
    utils::{webhooks::AutoResponderRequest, AutoResponder},
};
use std::{
    borrow::Cow,
    collections::{BTreeMap, VecDeque},
};
use time::OffsetDateTime;

pub struct AutoRespondersApi<'a> {
    primary_db: Cow<'a, PrimaryDb>,
}

impl<'a> AutoRespondersApi<'a> {
    /// Creates WebHooks API.
    pub fn new(primary_db: &'a PrimaryDb) -> Self {
        Self {
            primary_db: Cow::Borrowed(primary_db),
        }
    }

    pub async fn get_auto_responder(
        &self,
        user_id: UserId,
        name: &str,
    ) -> anyhow::Result<Option<AutoResponder>> {
        self.primary_db
            .get_user_data::<BTreeMap<String, AutoResponder>>(
                user_id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await
            .map(|auto_responders| auto_responders?.value.remove(name))
    }

    /// Tracks request to the specified auto responder.
    pub async fn track_request<'r>(
        &self,
        user_id: UserId,
        auto_responder: &AutoResponder,
        request: AutoResponderRequest<'_>,
    ) -> anyhow::Result<()> {
        let mut requests = self
            .primary_db
            .get_user_data::<VecDeque<AutoResponderRequest>>(
                user_id,
                (
                    PublicUserDataNamespace::AutoResponders,
                    auto_responder.name.as_str(),
                ),
            )
            .await?
            .map(|user_data| user_data.value)
            .unwrap_or_default();
        // Enforce request limit and displace the oldest one.
        if requests.len() == auto_responder.requests_to_track {
            requests.pop_front();
        }
        requests.push_back(request);

        self.primary_db
            .upsert_user_data(
                user_id,
                (
                    PublicUserDataNamespace::AutoResponders,
                    auto_responder.name.as_str(),
                ),
                UserData::new(requests, OffsetDateTime::now_utc()),
            )
            .await
    }

    /// Returns all requests to the specified auto responder that have been tracked.
    pub async fn get_requests(
        &self,
        user_id: UserId,
        auto_responder: &AutoResponder,
    ) -> anyhow::Result<Vec<AutoResponderRequest<'static>>> {
        Ok(self
            .primary_db
            .get_user_data::<VecDeque<AutoResponderRequest>>(
                user_id,
                (
                    PublicUserDataNamespace::AutoResponders,
                    auto_responder.name.as_str(),
                ),
            )
            .await?
            .map(|user_data| user_data.value)
            .unwrap_or_default()
            .into())
    }
}

impl Api {
    /// Returns an API to work with auto responders.
    pub fn auto_responders(&self) -> AutoRespondersApi {
        AutoRespondersApi::new(&self.datastore.primary_db)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        authentication::StoredCredentials,
        datastore::PrimaryDb,
        tests::MockUserBuilder,
        users::{User, UserId},
        utils::{
            webhooks::AutoRespondersApi, AutoResponder, AutoResponderMethod, AutoResponderRequest,
        },
    };
    use std::borrow::Cow;
    use time::OffsetDateTime;

    fn create_mock_user() -> User {
        MockUserBuilder::new(
            UserId(1),
            "dev@secutils.dev",
            "dev-handle",
            StoredCredentials {
                password_hash: Some("hash".to_string()),
                ..Default::default()
            },
            OffsetDateTime::now_utc(),
        )
        .build()
    }

    async fn initialize_mock_db(user: &User) -> anyhow::Result<PrimaryDb> {
        let db = PrimaryDb::open(|| Ok("sqlite::memory:".to_string())).await?;
        db.upsert_user(user).await.map(|_| db)
    }

    #[actix_rt::test]
    async fn properly_tracks_requests() -> anyhow::Result<()> {
        let mock_user = create_mock_user();
        let mock_db = initialize_mock_db(&mock_user).await?;
        let api = AutoRespondersApi::new(&mock_db);
        let auto_responder = AutoResponder {
            name: "name".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };

        let request_one = AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.1".parse()?),
            method: Cow::Borrowed("GET"),
            headers: Some(vec![(Cow::Borrowed("header"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        };
        let request_two = AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.2".parse()?),
            method: Cow::Borrowed("POST"),
            headers: Some(vec![(Cow::Borrowed("header"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        };
        let request_three = AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.3".parse()?),
            method: Cow::Borrowed("PUT"),
            headers: Some(vec![(Cow::Borrowed("header"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        };
        let request_four = AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.4".parse()?),
            method: Cow::Borrowed("DELETE"),
            headers: Some(vec![(Cow::Borrowed("header"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        };

        // Track requests within limit.
        api.track_request(mock_user.id, &auto_responder, request_one.clone())
            .await?;
        api.track_request(mock_user.id, &auto_responder, request_two.clone())
            .await?;
        api.track_request(mock_user.id, &auto_responder, request_three.clone())
            .await?;
        assert_eq!(
            api.get_requests(mock_user.id, &auto_responder).await?,
            vec![
                request_one.clone(),
                request_two.clone(),
                request_three.clone()
            ]
        );

        // Exceed limit.
        api.track_request(mock_user.id, &auto_responder, request_four.clone())
            .await?;
        assert_eq!(
            api.get_requests(mock_user.id, &auto_responder).await?,
            vec![request_two, request_three, request_four]
        );

        Ok(())
    }
}
