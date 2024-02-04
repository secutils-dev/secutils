use crate::{
    server::AppState,
    users::{UserShare, UserShareId},
};
use actix_web::{
    dev::Payload,
    error::{ErrorBadRequest, ErrorInternalServerError, ErrorUnauthorized},
    http::header::HeaderName,
    web, Error, FromRequest, HttpRequest,
};
use anyhow::anyhow;
use std::{future::Future, pin::Pin};

pub static USER_SHARE_ID_HEADER_NAME: HeaderName = HeaderName::from_static("x-user-share-id");

/// Extractor used to extract `UserShare` reference from the request via `X-Share-ID` HTTP header.
impl FromRequest for UserShare {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let req = req.clone();
        Box::pin(async move {
            // 1. Try to extract `X-Share-ID` header value.
            let header_value = if let Some(header) = req.headers().get(&USER_SHARE_ID_HEADER_NAME) {
                header
                    .to_str()
                    .map_err(|_| ErrorBadRequest(anyhow!("Invalid X-User-Share-ID header.")))?
            } else {
                return Err(ErrorUnauthorized(anyhow!(
                    "X-User-Share-ID header is missing."
                )));
            };

            // 2. Make sure that the header value is a valid `UserShareId` (UUIDv4).
            let user_share_id: UserShareId = header_value.parse().map_err(|err| {
                log::error!(
                    "Invalid X-User-Share-ID header `{}`: {:?}",
                    header_value,
                    err
                );
                ErrorBadRequest(anyhow!("Invalid X-User-Share-ID header."))
            })?;

            // 3. Retrieve `UserShare` from the database using the extracted `UserShareId`.
            let state = web::Data::<AppState>::extract(&req).await?;
            let users = state.api.users();
            let user_share = users.get_user_share(user_share_id).await.map_err(|err| {
                log::error!(
                    "Cannot retrieve user share ({}) due to unexpected error: {:?}.",
                    *user_share_id,
                    err
                );
                ErrorInternalServerError(anyhow!("Internal server error"))
            })?;

            // 4. Make sure that the `UserShare` is still available, otherwise fail with an error.
            user_share.ok_or_else(|| {
                log::error!(
                    "Tried to access unavailable user share ({}).",
                    *user_share_id
                );
                ErrorUnauthorized(anyhow!(
                    "X-User-Share-ID header points to non-existent user share."
                ))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::USER_SHARE_ID_HEADER_NAME;
    use crate::{
        tests::{mock_app_state, mock_user},
        users::{SharedResource, UserShare, UserShareId},
    };
    use actix_web::{dev::Payload, test::TestRequest, FromRequest};
    use insta::assert_debug_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[tokio::test]
    async fn fails_if_header_is_not_provided() -> anyhow::Result<()> {
        let request = TestRequest::default().to_http_request();
        assert_debug_snapshot!(UserShare::from_request(&request, &mut Payload::None).await, @r###"
        Err(
            X-User-Share-ID header is missing.,
        )
        "###);
        Ok(())
    }

    #[tokio::test]
    async fn fails_if_header_is_not_valid() -> anyhow::Result<()> {
        let request = TestRequest::default()
            .insert_header((USER_SHARE_ID_HEADER_NAME.clone(), "xxx"))
            .to_http_request();
        assert_debug_snapshot!(UserShare::from_request(&request, &mut Payload::None).await, @r###"
        Err(
            Invalid X-User-Share-ID header.,
        )
        "###);
        Ok(())
    }

    #[tokio::test]
    async fn fails_if_user_share_is_not_available() -> anyhow::Result<()> {
        let user = mock_user()?;
        let mock_user_share = UserShare {
            id: UserShareId::new(),
            user_id: user.id,
            resource: SharedResource::ContentSecurityPolicy {
                policy_id: uuid!("00000000-0000-0000-0000-000000000000"),
            },
            created_at: OffsetDateTime::now_utc(),
        };

        let app_state = mock_app_state().await?;
        let users = app_state.api.users();
        users.upsert(&user).await?;

        let request = TestRequest::default()
            .insert_header((
                USER_SHARE_ID_HEADER_NAME.clone(),
                mock_user_share.id.hyphenated().to_string(),
            ))
            .data(app_state)
            .to_http_request();
        assert_debug_snapshot!(UserShare::from_request(&request, &mut Payload::None).await, @r###"
        Err(
            X-User-Share-ID header points to non-existent user share.,
        )
        "###);
        Ok(())
    }

    #[tokio::test]
    async fn can_extract_user_share() -> anyhow::Result<()> {
        let user = mock_user()?;
        let mock_user_share = UserShare {
            id: UserShareId::new(),
            user_id: user.id,
            resource: SharedResource::ContentSecurityPolicy {
                policy_id: uuid!("00000000-0000-0000-0000-000000000000"),
            },
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
        };

        let app_state = mock_app_state().await?;
        let users = app_state.api.users();
        users.upsert(&user).await?;
        users.insert_user_share(&mock_user_share).await?;

        let request = TestRequest::default()
            .insert_header((
                USER_SHARE_ID_HEADER_NAME.clone(),
                mock_user_share.id.hyphenated().to_string(),
            ))
            .data(app_state)
            .to_http_request();
        assert_eq!(
            UserShare::from_request(&request, &mut Payload::None)
                .await
                .unwrap(),
            mock_user_share
        );
        Ok(())
    }
}
