use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{DictionaryDataUserDataSetter, PublicUserDataNamespace, UserData, UserId},
    utils::{webhooks::AutoResponderRequest, AutoResponder},
};
use anyhow::bail;
use std::collections::{BTreeMap, VecDeque};
use time::OffsetDateTime;

pub struct AutoRespondersApi<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> AutoRespondersApi<'a, DR, ET> {
    /// Creates AutoResponders API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Returns auto responder by its path.
    pub async fn get_auto_responder(
        &self,
        user_id: UserId,
        responder_path: &str,
    ) -> anyhow::Result<Option<AutoResponder>> {
        let users_api = self.api.users();
        Ok(users_api
            .get_data::<BTreeMap<String, AutoResponder>>(
                user_id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await?
            .and_then(|mut map| map.value.remove(responder_path)))
    }

    /// Upserts auto responder.
    pub async fn upsert_auto_responder(
        &self,
        user_id: UserId,
        responder: AutoResponder,
    ) -> anyhow::Result<()> {
        if !responder.is_valid() {
            bail!(
                "User ({}) responder ({}) is not valid: {responder:?}",
                *user_id,
                responder.path
            );
        }

        DictionaryDataUserDataSetter::upsert(
            &self.api.db,
            PublicUserDataNamespace::AutoResponders,
            UserData::new(
                user_id,
                [(responder.path.clone(), Some(responder))]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
                OffsetDateTime::now_utc(),
            ),
        )
        .await?;

        Ok(())
    }

    /// Removes auto responder by its path and returns it.
    pub async fn remove_auto_responder(
        &self,
        user_id: UserId,
        responder_path: &str,
    ) -> anyhow::Result<()> {
        // Remove responder requests.
        self.api
            .db
            .remove_user_data(
                user_id,
                (PublicUserDataNamespace::AutoResponders, responder_path),
            )
            .await?;

        DictionaryDataUserDataSetter::upsert(
            &self.api.db,
            PublicUserDataNamespace::AutoResponders,
            UserData::new(
                user_id,
                [(responder_path.to_string(), None)]
                    .into_iter()
                    .collect::<BTreeMap<_, Option<AutoResponder>>>(),
                OffsetDateTime::now_utc(),
            ),
        )
        .await
    }

    /// Tracks request to the specified auto responder.
    pub async fn track_request<'r>(
        &self,
        user_id: UserId,
        auto_responder: &AutoResponder,
        request: AutoResponderRequest<'_>,
    ) -> anyhow::Result<()> {
        let mut requests = self
            .api
            .db
            .get_user_data::<VecDeque<AutoResponderRequest>>(
                user_id,
                (
                    PublicUserDataNamespace::AutoResponders,
                    auto_responder.path.as_str(),
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

        self.api
            .db
            .upsert_user_data(
                (
                    PublicUserDataNamespace::AutoResponders,
                    auto_responder.path.as_str(),
                ),
                UserData::new_with_key(
                    user_id,
                    &auto_responder.path,
                    requests,
                    OffsetDateTime::now_utc(),
                ),
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
            .api
            .db
            .get_user_data::<VecDeque<AutoResponderRequest>>(
                user_id,
                (
                    PublicUserDataNamespace::AutoResponders,
                    auto_responder.path.as_str(),
                ),
            )
            .await?
            .map(|user_data| user_data.value)
            .unwrap_or_default()
            .into())
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with auto responders.
    pub fn auto_responders(&self) -> AutoRespondersApi<DR, ET> {
        AutoRespondersApi::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::{mock_api, mock_user},
        users::PublicUserDataNamespace,
        utils::{
            webhooks::AutoRespondersApi, AutoResponder, AutoResponderMethod, AutoResponderRequest,
        },
    };
    use std::{borrow::Cow, collections::BTreeMap};
    use time::OffsetDateTime;

    #[actix_rt::test]
    async fn properly_saves_new_responders() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let auto_responders = AutoRespondersApi::new(&api);
        let responder_one = AutoResponder {
            path: "/name-one".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };
        auto_responders
            .upsert_auto_responder(mock_user.id, responder_one.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<BTreeMap<String, AutoResponder>>(
                mock_user.id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(responder_one.path.clone(), responder_one.clone())]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
        );
        assert_eq!(
            auto_responders
                .get_auto_responder(mock_user.id, &responder_one.path)
                .await?,
            Some(responder_one.clone())
        );

        let responder_two = AutoResponder {
            path: "/name-two".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };
        auto_responders
            .upsert_auto_responder(mock_user.id, responder_two.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<BTreeMap<String, AutoResponder>>(
                mock_user.id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [
                (responder_one.path.clone(), responder_one.clone()),
                (responder_two.path.clone(), responder_two.clone())
            ]
            .into_iter()
            .collect::<BTreeMap<_, _>>()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_updates_existing_responders() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let auto_responders = AutoRespondersApi::new(&api);
        let responder_one = AutoResponder {
            path: "/name-one".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };
        auto_responders
            .upsert_auto_responder(mock_user.id, responder_one.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<BTreeMap<String, AutoResponder>>(
                mock_user.id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(responder_one.path.clone(), responder_one.clone())]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
        );

        let responder_one = AutoResponder {
            path: "/name-one".to_string(),
            method: AutoResponderMethod::Post,
            requests_to_track: 3,
            status_code: 300,
            body: None,
            headers: None,
            delay: None,
        };
        auto_responders
            .upsert_auto_responder(mock_user.id, responder_one.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<BTreeMap<String, AutoResponder>>(
                mock_user.id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(responder_one.path.clone(), responder_one.clone())]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_removes_responders() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let responder_one = AutoResponder {
            path: "/name-one".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };
        let responder_two = AutoResponder {
            path: "/name-two".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };

        let auto_responders = AutoRespondersApi::new(&api);
        auto_responders
            .upsert_auto_responder(mock_user.id, responder_one.clone())
            .await?;
        auto_responders
            .upsert_auto_responder(mock_user.id, responder_two.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<BTreeMap<String, AutoResponder>>(
                mock_user.id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [
                (responder_one.path.clone(), responder_one.clone()),
                (responder_two.path.clone(), responder_two.clone())
            ]
            .into_iter()
            .collect::<BTreeMap<_, _>>()
        );

        auto_responders
            .remove_auto_responder(mock_user.id, &responder_one.path)
            .await?;

        let user_data = api
            .users()
            .get_data::<BTreeMap<String, AutoResponder>>(
                mock_user.id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(responder_two.path.clone(), responder_two.clone())]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
        );

        auto_responders
            .remove_auto_responder(mock_user.id, &responder_two.path)
            .await?;

        let user_data = api
            .users()
            .get_data::<BTreeMap<String, AutoResponder>>(
                mock_user.id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await?;
        assert!(user_data.is_none());

        Ok(())
    }

    #[actix_rt::test]
    async fn properly_tracks_requests() -> anyhow::Result<()> {
        let mock_user = mock_user()?;
        let global_api = mock_api().await?;
        global_api.db.upsert_user(&mock_user).await?;

        let api = AutoRespondersApi::new(&global_api);
        let auto_responder = AutoResponder {
            path: "/name".to_string(),
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

    #[actix_rt::test]
    async fn properly_removes_requests_when_responder_is_removed() -> anyhow::Result<()> {
        let api = mock_api().await?;

        let mock_user = mock_user()?;
        api.db.insert_user(&mock_user).await?;

        let responder = AutoResponder {
            path: "/name-one".to_string(),
            method: AutoResponderMethod::Any,
            requests_to_track: 3,
            status_code: 200,
            body: None,
            headers: None,
            delay: None,
        };

        // Create responder.
        let auto_responders = AutoRespondersApi::new(&api);
        auto_responders
            .upsert_auto_responder(mock_user.id, responder.clone())
            .await?;

        // Track request.
        let request = AutoResponderRequest {
            // January 1, 2000 11:00:00
            timestamp: OffsetDateTime::from_unix_timestamp(946720800)?,
            client_address: Some("127.0.0.1".parse()?),
            method: Cow::Borrowed("GET"),
            headers: Some(vec![(Cow::Borrowed("header"), Cow::Borrowed(&[1, 2, 3]))]),
            body: Some(Cow::Borrowed(&[4, 5, 6])),
        };
        auto_responders
            .track_request(mock_user.id, &responder, request.clone())
            .await?;

        let user_data = api
            .users()
            .get_data::<BTreeMap<String, AutoResponder>>(
                mock_user.id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await?
            .unwrap();
        assert_eq!(
            user_data.value,
            [(responder.path.clone(), responder.clone()),]
                .into_iter()
                .collect::<BTreeMap<_, _>>()
        );

        assert_eq!(
            auto_responders
                .get_requests(mock_user.id, &responder)
                .await?,
            vec![request.clone()]
        );

        auto_responders
            .remove_auto_responder(mock_user.id, &responder.path)
            .await?;

        let user_data = api
            .users()
            .get_data::<BTreeMap<String, AutoResponder>>(
                mock_user.id,
                PublicUserDataNamespace::AutoResponders,
            )
            .await?;
        assert!(user_data.is_none());
        assert_eq!(
            auto_responders
                .get_requests(mock_user.id, &responder)
                .await?,
            vec![]
        );

        Ok(())
    }
}
