use crate::{
    security::{Operator, SecurityApiExt, kratos::RecoveryLink},
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::{SubscriptionTier, User, UserDataCloneSummary, UserId, UserSubscription},
};
use actix_web::{HttpResponse, post, web};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use std::{ops::Add, time::Duration};
use tracing::{error, info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

/// Selects which existing user to clone from.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum CloneSource {
    Email(String),
    Id(Uuid),
}

/// Destination credentials for the new clone.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloneDestination {
    /// Email address for the new clone.
    pub email: String,
    /// Optional handle override. When omitted, a fresh random handle is generated.
    #[serde(default)]
    pub handle: Option<String>,
}

/// Parameters for the clone request.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "source": { "email": "user@example.com" },
    "destination": { "email": "user-clone@secutils.dev" },
    "includeHistory": true,
    "copySubscription": true,
    "recoveryLinkExpiresIn": "1h"
}))]
pub struct CloneParams {
    /// Existing user to clone data from (looked up by ID or email).
    pub source: CloneSource,
    /// Email + optional handle for the new clone.
    pub destination: CloneDestination,
    /// Whether to copy responder request history and tracker revision history (default true).
    #[serde(default = "default_include_history")]
    pub include_history: bool,
    /// Whether the clone inherits the source user's subscription tier so feature gating
    /// behaves identically. When false, the clone gets a fresh basic-tier trial.
    #[serde(default = "default_copy_subscription")]
    pub copy_subscription: bool,
    /// Kratos recovery-link lifetime as a humantime-style string (e.g. `"1h"`, `"15m"`,
    /// `"30s"`). Bounded by Kratos's own configured maximum (typically 15 m). Defaults to 1 h.
    #[serde(
        default = "default_recovery_link_expires_in",
        deserialize_with = "humantime_duration::deserialize"
    )]
    #[schema(value_type = String, example = "1h")]
    pub recovery_link_expires_in: Duration,
}

fn default_include_history() -> bool {
    true
}

fn default_copy_subscription() -> bool {
    true
}

fn default_recovery_link_expires_in() -> Duration {
    Duration::from_secs(60 * 60)
}

/// Serde shim that parses a humantime-formatted string (e.g. `"1h"`, `"15m"`, `"30s"`, `"1h 30m"`)
/// into a `Duration`. Kept as a one-way `deserialize_with` because the field is consumer-only (the
/// struct doesn't need `Serialize`), the same humantime text format is what Kratos's `expires_in`
/// admin parameter accepts, so the handler re-formats the `Duration` back to a string with
/// `humantime::format_duration` at the call site.
mod humantime_duration {
    use super::{Deserialize, Deserializer, Duration};
    use serde::de::Error as DeError;

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        let raw = String::deserialize(deserializer)?;
        humantime::parse_duration(&raw).map_err(DeError::custom)
    }
}

/// Successful clone response.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloneResponse {
    /// UUID of the newly created destination user. Exposed at the top level because [`User`] elides
    /// its `id` field from serialisation, but operator tooling needs it (e.g. to call
    /// `DELETE /api/users/{user_id}` later).
    #[schema(value_type = String, format = Uuid)]
    pub id: UserId,
    /// The newly created destination user.
    #[schema(value_type = Object)]
    pub user: User,
    /// Single-use Kratos URL the operator follows to set a password and complete login.
    pub recovery_link: RecoveryLink,
    /// Per-entity-type summary of what was copied from source to destination. Mirrors the
    /// shape returned by `POST /api/user/data/_import`.
    #[schema(value_type = Object)]
    pub summary: UserDataCloneSummary,
}

/// Clones a user (operator-only).
///
/// Creates a fresh, email-verified Kratos identity for the destination email, copies every piece of
/// source data (tags, scripts, secrets, responders, certificate templates, private keys, content
/// security policies, page/API trackers, settings, optionally including history) under regenerated
/// IDs, and returns a Kratos-issued recovery link the operator clicks to set a password and log in
/// as the clone.
///
/// Use this to reproduce a customer's issue without ever touching their live state. After
/// debugging, remove the clone via `DELETE /api/users/{user_id}` or `POST /api/users/remove`.
#[utoipa::path(
    tags = ["users"],
    request_body = CloneParams,
    responses(
        (status = 200, description = "Clone was successfully created.", body = CloneResponse),
        (status = BAD_REQUEST, description = "Invalid parameters or destination already registered."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = FORBIDDEN, description = "Caller is not an operator."),
        (status = NOT_FOUND, description = "Source user not found.")
    )
)]
#[post("/api/users/_clone")]
pub async fn security_users_clone(
    state: web::Data<AppState>,
    operator: Operator,
    body_params: web::Json<CloneParams>,
) -> Result<HttpResponse, actix_web::Error> {
    clone_inner(&state, &operator, body_params.into_inner()).await
}

/// Concrete-typed core of the [`security_users_clone`] handler, lifted out so unit tests
/// can call it directly without spinning up an actix `App` or going through the
/// `Operator` extractor. Returns the same `HttpResponse` shape the actix handler does.
pub(super) async fn clone_inner<DR, ET>(
    state: &AppState<DR, ET>,
    operator: &Operator,
    params: CloneParams,
) -> Result<HttpResponse, actix_web::Error>
where
    DR: crate::network::DnsResolver,
    ET: crate::network::EmailTransport,
    ET::Error: crate::network::EmailTransportError,
{
    let destination_email = params.destination.email.trim().to_lowercase();
    if destination_email.is_empty() {
        return Ok(HttpResponse::BadRequest()
            .json(json!({ "message": "Destination email cannot be empty." })));
    }

    let security_api = state.api.security();
    let users_api = state.api.users();

    // 1. Resolve source user.
    let source_lookup = match &params.source {
        CloneSource::Id(id) => users_api.get(UserId::from(*id)).await,
        CloneSource::Email(email) => users_api.get_by_email(email).await,
    };
    let source = match source_lookup {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!(
                operator = operator.id(),
                "Cannot clone non-existent source user."
            );
            return Ok(
                HttpResponse::NotFound().json(json!({ "message": "Source user not found." }))
            );
        }
        Err(err) => {
            error!(
                operator = operator.id(),
                "Failed to look up source user for clone: {err:?}"
            );
            return Ok(generic_internal_server_error());
        }
    };

    // 2. Check destination email is not already registered (cheap pre-check before we touch
    // Kratos). The Kratos create-identity call enforces uniqueness too, but failing here gives the
    // operator a clearer 400 response.
    match users_api.get_by_email(&destination_email).await {
        Ok(Some(_)) => {
            return Ok(HttpResponse::BadRequest().json(json!({
                "message": "Destination email is already registered. Remove the existing user or pick a different email."
            })));
        }
        Ok(None) => {}
        Err(err) => {
            error!(
                operator = operator.id(),
                "Failed to check destination email availability: {err:?}"
            );
            return Ok(generic_internal_server_error());
        }
    }

    // 3. Create Kratos identity (verified, no credentials, recovery link will let operator set a
    // password).
    let identity = match security_api.create_identity(&destination_email, true).await {
        Ok(identity) => identity,
        Err(err) => {
            error!(
                operator = operator.id(),
                "Failed to create Kratos identity for clone: {err:?}"
            );
            return Ok(generic_internal_server_error());
        }
    };

    // 4. Build destination User. From this point on, any error path must terminate the half-created
    // clone to avoid orphaning a Kratos identity or a Postgres row.
    let destination_handle = match params.destination.handle.clone() {
        Some(handle) if !handle.trim().is_empty() => handle.trim().to_string(),
        _ => match security_api.generate_user_handle().await {
            Ok(handle) => handle,
            Err(err) => {
                error!(
                    operator = operator.id(),
                    "Failed to generate handle for clone: {err:?}"
                );
                rollback(&security_api, &destination_email, operator.id()).await;
                return Ok(generic_internal_server_error());
            }
        },
    };

    let trial_end = identity.created_at.add(UserSubscription::TRIAL_LENGTH);
    let subscription = if params.copy_subscription {
        // Inherit the source user's subscription tier so feature gating matches; restart the
        // started_at clock to the identity's creation moment to avoid "subscription started in
        // the past" confusion in the UI.
        UserSubscription {
            tier: source.subscription.tier,
            started_at: identity.created_at,
            ends_at: source.subscription.ends_at,
            trial_started_at: Some(identity.created_at),
            trial_ends_at: Some(trial_end),
        }
    } else {
        UserSubscription {
            tier: SubscriptionTier::Basic,
            started_at: identity.created_at,
            ends_at: None,
            trial_started_at: Some(identity.created_at),
            trial_ends_at: Some(trial_end),
        }
    };

    let destination = User {
        id: UserId::from(identity.id),
        email: destination_email.clone(),
        handle: destination_handle,
        created_at: identity.created_at,
        is_activated: identity.is_activated(),
        is_operator: false,
        subscription,
    };

    // 5. Insert the destination user row.
    if let Err(err) = security_api.signup(&destination).await {
        error!(
            operator = operator.id(),
            user.id = %destination.id,
            "Failed to sign up destination user during clone: {err:?}"
        );
        rollback(&security_api, &destination_email, operator.id()).await;
        return Ok(generic_internal_server_error());
    }

    // 6. Copy all source data into the destination.
    let summary = match security_api
        .clone_data(&source, &destination, params.include_history)
        .await
    {
        Ok(summary) => summary,
        Err(err) => {
            error!(
                operator = operator.id(),
                source.id = %source.id,
                destination.id = %destination.id,
                "Failed to copy data during clone: {err:?}"
            );
            rollback(&security_api, &destination_email, operator.id()).await;
            return Ok(generic_internal_server_error());
        }
    };

    // 7. Mint the recovery link.
    let expires_in = humantime::format_duration(params.recovery_link_expires_in).to_string();
    let recovery_link = match security_api
        .create_recovery_link(identity.id, &expires_in)
        .await
    {
        Ok(link) => link,
        Err(err) => {
            error!(
                operator = operator.id(),
                destination.id = %destination.id,
                "Failed to mint recovery link for clone: {err:?}"
            );
            rollback(&security_api, &destination_email, operator.id()).await;
            return Ok(generic_internal_server_error());
        }
    };

    info!(
        operator = operator.id(),
        source.id = %source.id,
        destination.id = %destination.id,
        "Successfully cloned user."
    );

    Ok(HttpResponse::Ok().json(CloneResponse {
        id: destination.id,
        user: destination,
        recovery_link,
        summary,
    }))
}

/// Best-effort cleanup of a half-created clone. Logs but never surfaces secondary failures the
/// primary error is what matters to the caller. Mirrors the rollback pattern used by `terminate()`
/// itself (Kratos identity delete + DB user delete).
async fn rollback<DR, ET>(
    security_api: &SecurityApiExt<'_, DR, ET>,
    destination_email: &str,
    operator_id: &str,
) where
    DR: crate::network::DnsResolver,
    ET: crate::network::EmailTransport,
    ET::Error: crate::network::EmailTransportError,
{
    match security_api.terminate(destination_email).await {
        Ok(Some(user_id)) => {
            warn!(
                operator = operator_id,
                user.id = %user_id,
                "Rolled back partially created clone."
            );
        }
        Ok(None) => {
            warn!(
                operator = operator_id,
                "Rollback found nothing to clean up for partially created clone."
            );
        }
        Err(err) => {
            error!(
                operator = operator_id,
                "Failed to roll back partially created clone: {err:?}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CloneDestination, CloneParams, CloneSource, clone_inner};
    use crate::{
        security::Operator,
        server::AppState,
        tests::{mock_api_with_config, mock_config, mock_user_with_id, schema_example},
    };
    use actix_web::{body::MessageBody, http::StatusCode};
    use httpmock::MockServer;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use std::{sync::Arc, time::Duration};
    use url::Url;
    use uuid::{Uuid, uuid};

    /// Shared Kratos identity payload (the same shape `mock_identity_json` uses in
    /// `api_ext.rs`), kept local so tests don't reach across modules.
    fn identity_json(user_id: &str, email: &str) -> Value {
        json!({
            "id": user_id,
            "traits": { "email": email },
            "verifiable_addresses": [{ "value": email, "verified": true }],
            "created_at": "2025-01-01T11:00:00Z"
        })
    }

    /// Builds a `CloneParams` that sources by email and targets the given destination
    /// email - the most common shape exercised by the rollback tests.
    fn params(source_email: &str, destination_email: &str) -> CloneParams {
        CloneParams {
            source: CloneSource::Email(source_email.to_string()),
            destination: CloneDestination {
                email: destination_email.to_string(),
                handle: Some("clonehandle".to_string()),
            },
            include_history: false,
            copy_subscription: false,
            recovery_link_expires_in: Duration::from_secs(60 * 60),
        }
    }

    /// Builds an `AppState<DR, ET>` over the same DNS/email mocks `mock_api_with_config`
    /// returns, with Kratos pointed at the supplied mock server.
    async fn app_state(
        pool: PgPool,
        kratos_admin_url: &str,
    ) -> anyhow::Result<
        AppState<crate::tests::MockResolver, lettre::transport::stub::AsyncStubTransport>,
    > {
        let mut config = mock_config()?;
        config.components.kratos_admin_url = Url::parse(kratos_admin_url)?;
        // The mock server doubles as Retrack: `clone_data` calls `generate_export`, which
        // always issues `GET /api/trackers` against `config.retrack.host` regardless of
        // whether the source has any trackers. Tests that reach the clone-data stage rely
        // on this catch-all 200/[] response (registered alongside the Kratos mocks at the
        // call site - see `rolls_back_when_recovery_link_minting_fails` and
        // `happy_path_returns_id_and_recovery_link`).
        config.retrack.host = Url::parse(kratos_admin_url)?;
        // Required so cloned secrets can be re-encrypted in clone_data() runs.
        config.security.secrets_encryption_key =
            Some("a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2".to_string());
        let api = mock_api_with_config(pool, config).await?;
        let cloned_config = api.config.clone();
        Ok(AppState::new(cloned_config, Arc::new(api)))
    }

    /// Reads the JSON body off a (non-streaming) `HttpResponse`.
    fn body_json(response: actix_web::HttpResponse) -> Value {
        let bytes = response.into_body().try_into_bytes().unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn clone_params_example_is_valid() {
        let example: CloneParams = serde_json::from_value(schema_example::<CloneParams>()).unwrap();
        assert!(!example.destination.email.is_empty());
        assert!(example.destination.handle.is_none());
        assert!(example.include_history);
        assert!(example.copy_subscription);
        assert_eq!(
            example.recovery_link_expires_in,
            Duration::from_secs(60 * 60)
        );
        match example.source {
            CloneSource::Email(email) => assert_eq!(email, "user@example.com"),
            CloneSource::Id(_) => panic!("Expected email source in example"),
        }
    }

    #[test]
    fn deserialize_source_by_email() {
        let source: CloneSource =
            serde_json::from_value(json!({ "email": "user@example.com" })).unwrap();
        match source {
            CloneSource::Email(email) => assert_eq!(email, "user@example.com"),
            CloneSource::Id(_) => panic!("Expected email source"),
        }
    }

    #[test]
    fn deserialize_source_by_id() {
        let id = uuid!("11111111-1111-1111-1111-111111111111");
        let source: CloneSource = serde_json::from_value(json!({ "id": id.to_string() })).unwrap();
        match source {
            CloneSource::Id(parsed) => assert_eq!(parsed, id),
            CloneSource::Email(_) => panic!("Expected id source"),
        }
    }

    #[test]
    fn deserialize_params_with_handle_override() {
        let params: CloneParams = serde_json::from_value(json!({
            "source": { "email": "user@example.com" },
            "destination": {
                "email": "clone@secutils.dev",
                "handle": "myclone"
            }
        }))
        .unwrap();
        assert_eq!(params.destination.email, "clone@secutils.dev");
        assert_eq!(params.destination.handle.as_deref(), Some("myclone"));
        // Defaults should apply when fields are omitted.
        assert!(params.include_history);
        assert!(params.copy_subscription);
        assert_eq!(
            params.recovery_link_expires_in,
            Duration::from_secs(60 * 60)
        );
    }

    #[test]
    fn deserialize_params_accepts_humantime_expires_in() {
        let cases = [
            ("1h", Duration::from_secs(60 * 60)),
            ("15m", Duration::from_secs(15 * 60)),
            ("30s", Duration::from_secs(30)),
            ("1h 30m", Duration::from_secs(90 * 60)),
        ];
        for (input, expected) in cases {
            let parsed: CloneParams = serde_json::from_value(json!({
                "source": { "email": "u@e.com" },
                "destination": { "email": "c@e.com" },
                "recoveryLinkExpiresIn": input,
            }))
            .unwrap();
            assert_eq!(parsed.recovery_link_expires_in, expected, "input: {input}");
        }
    }

    #[test]
    fn deserialize_params_rejects_malformed_expires_in() {
        let result: Result<CloneParams, _> = serde_json::from_value(json!({
            "source": { "email": "u@e.com" },
            "destination": { "email": "c@e.com" },
            "recoveryLinkExpiresIn": "not-a-duration",
        }));
        assert!(result.is_err(), "should reject garbage humantime values");
    }

    /// "Tripwire" mock that 500s on any Kratos admin call. If a code path is supposed to
    /// avoid Kratos entirely (e.g. early validation failures), attach this and assert it
    /// recorded zero calls.
    fn kratos_tripwire(server: &MockServer) -> httpmock::Mock<'_> {
        server.mock(|when, then| {
            when.path_matches(regex::Regex::new("^/admin/").unwrap());
            then.status(500).body("unexpected Kratos call");
        })
    }

    /// Confirms the empty-destination guard short-circuits before any external work.
    #[sqlx::test]
    async fn returns_400_for_empty_destination_email(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        let tripwire = kratos_tripwire(&server);
        let state = app_state(pool, &server.base_url()).await?;
        let op = Operator::new("@secutils");

        let mut p = params("source@secutils.dev", "");
        p.destination.email = "   ".to_string();

        let resp = clone_inner(&state, &op, p).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        tripwire.assert_calls(0);
        Ok(())
    }

    /// Source lookup miss must return 404 without ever touching Kratos.
    #[sqlx::test]
    async fn returns_404_when_source_user_missing(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        let tripwire = kratos_tripwire(&server);
        let state = app_state(pool, &server.base_url()).await?;
        let op = Operator::new("@secutils");

        let resp = clone_inner(
            &state,
            &op,
            params("nobody@secutils.dev", "clone@secutils.dev"),
        )
        .await
        .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        tripwire.assert_calls(0);
        Ok(())
    }

    /// Pre-flight destination-email collision check beats Kratos to the punch.
    #[sqlx::test]
    async fn returns_400_when_destination_already_registered(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        let tripwire = kratos_tripwire(&server);
        let state = app_state(pool, &server.base_url()).await?;
        let op = Operator::new("@secutils");

        let source = mock_user_with_id(uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"))?;
        let existing_destination =
            mock_user_with_id(uuid!("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"))?;
        state.api.db.insert_user(&source).await?;
        state.api.db.insert_user(&existing_destination).await?;

        let resp = clone_inner(
            &state,
            &op,
            params(&source.email, &existing_destination.email),
        )
        .await
        .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        tripwire.assert_calls(0);
        Ok(())
    }

    /// Identity-creation failure: nothing was inserted, so rollback shouldn't fire either.
    #[sqlx::test]
    async fn returns_500_and_no_db_row_when_identity_creation_fails(
        pool: PgPool,
    ) -> anyhow::Result<()> {
        let server = MockServer::start();
        let identity_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/admin/identities");
            then.status(500).body("kratos exploded");
        });

        let state = app_state(pool, &server.base_url()).await?;
        let op = Operator::new("@secutils");
        let source = mock_user_with_id(uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"))?;
        state.api.db.insert_user(&source).await?;

        let resp = clone_inner(&state, &op, params(&source.email, "clone@secutils.dev"))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        identity_mock.assert_calls(1);
        // Destination user must not exist in the DB.
        assert!(
            state
                .api
                .users()
                .get_by_email("clone@secutils.dev")
                .await?
                .is_none()
        );
        Ok(())
    }

    /// Signup-failure path (we already inserted the user up-front, so calling `signup` a second
    /// time fails the duplicate-check). Verifies rollback runs and clears the Kratos identity.
    #[sqlx::test]
    async fn rolls_back_when_signup_fails(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();

        let destination_email = "clone@secutils.dev";
        let destination_id = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

        let create_identity_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/admin/identities");
            then.status(201)
                .header("Content-Type", "application/json")
                .json_body(identity_json(destination_id, destination_email));
        });
        let lookup_identity_mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/admin/identities");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!([identity_json(destination_id, destination_email)]));
        });
        let delete_identity_mock = server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/admin/identities/{destination_id}"));
            then.status(204);
        });

        let state = app_state(pool, &server.base_url()).await?;
        let op = Operator::new("@secutils");

        let source = mock_user_with_id(uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"))?;
        state.api.db.insert_user(&source).await?;
        // Pre-insert a row with the SAME id Kratos will return, so the signup duplicate-check
        // in `SecurityApiExt::signup` will fail and trigger the rollback path.
        let preexisting = mock_user_with_id(Uuid::parse_str(destination_id)?)?;
        state.api.db.insert_user(&preexisting).await?;

        let resp = clone_inner(&state, &op, params(&source.email, destination_email))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        create_identity_mock.assert_calls(1);
        // Rollback ran: identity-by-email lookup + DELETE.
        lookup_identity_mock.assert_calls(1);
        delete_identity_mock.assert_calls(1);
        Ok(())
    }

    /// Recovery-link minting failure must trigger full rollback (Kratos identity + DB row).
    #[sqlx::test]
    async fn rolls_back_when_recovery_link_minting_fails(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        let destination_email = "clone@secutils.dev";
        let destination_id = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

        server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/admin/identities");
            then.status(201)
                .header("Content-Type", "application/json")
                .json_body(identity_json(destination_id, destination_email));
        });
        // Retrack tracker-list call made by `clone_data` -> `generate_export`. Returning an
        // empty array lets clone_data succeed, so the failure mode under test (recovery-link
        // minting) is the one that actually trips the rollback.
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!([]));
        });
        let recovery_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/admin/recovery/code");
            then.status(500).body("kratos recovery unavailable");
        });
        let lookup_mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/admin/identities");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!([identity_json(destination_id, destination_email)]));
        });
        let delete_mock = server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/admin/identities/{destination_id}"));
            then.status(204);
        });

        let state = app_state(pool, &server.base_url()).await?;
        let op = Operator::new("@secutils");

        let source = mock_user_with_id(uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"))?;
        state.api.db.insert_user(&source).await?;

        let resp = clone_inner(&state, &op, params(&source.email, destination_email))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        recovery_mock.assert_calls(1);
        lookup_mock.assert_calls(1);
        delete_mock.assert_calls(1);
        // DB row was rolled back too.
        assert!(
            state
                .api
                .users()
                .get_by_email(destination_email)
                .await?
                .is_none()
        );
        Ok(())
    }

    /// Happy path - all Kratos calls succeed, no rollback runs, response carries the new id
    /// at the top level (because `User` skips serialising its own id).
    #[sqlx::test]
    async fn happy_path_returns_id_and_recovery_link(pool: PgPool) -> anyhow::Result<()> {
        let server = MockServer::start();
        let destination_email = "clone@secutils.dev";
        let destination_id = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
        let expected_link =
            "http://127.0.0.1:7171/self-service/recovery?flow=abc&token=xyz".to_string();

        server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/admin/identities");
            then.status(201)
                .header("Content-Type", "application/json")
                .json_body(identity_json(destination_id, destination_email));
        });
        // Retrack tracker-list call made by `clone_data` -> `generate_export`. Source user
        // has no trackers so an empty array is the correct response.
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/api/trackers");
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!([]));
        });
        let recovery_mock = server.mock(|when, then| {
            when.method(httpmock::Method::POST)
                .path("/admin/recovery/code")
                // Kratos expects humantime-style expires_in - confirm we pass it through.
                .json_body_includes(json!({ "expires_in": "1h" }).to_string());
            then.status(200)
                .header("Content-Type", "application/json")
                .json_body(json!({
                    "recovery_link": expected_link,
                    "recovery_code": "123456",
                    "expires_at": "2030-01-01T10:00:00Z"
                }));
        });
        let delete_mock = server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path(format!("/admin/identities/{destination_id}"));
            then.status(204);
        });

        let state = app_state(pool, &server.base_url()).await?;
        let op = Operator::new("@secutils");
        let source = mock_user_with_id(uuid!("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"))?;
        state.api.db.insert_user(&source).await?;

        let resp = clone_inner(&state, &op, params(&source.email, destination_email))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = body_json(resp);
        // Top-level id matches the Kratos identity id and feeds e.g. DELETE /api/users/{id}.
        // `User` skips serialising its own id field - the top-level `id` is the operator's only
        // handle on the new clone, so pin both that and the absence of `user.id`.
        assert_eq!(
            body["id"].as_str(),
            Some(destination_id),
            "top-level id must mirror the Kratos identity uuid"
        );
        assert!(
            body["user"].get("id").is_none(),
            "User struct must not leak its id into the nested object"
        );
        assert_eq!(body["user"]["email"].as_str(), Some(destination_email));
        assert_eq!(
            body["recoveryLink"]["recovery_link"].as_str(),
            Some(expected_link.as_str())
        );

        recovery_mock.assert_calls(1);
        // No rollback path was taken on the happy path.
        delete_mock.assert_calls(0);
        // Destination row landed in the DB.
        assert!(
            state
                .api
                .users()
                .get_by_email(destination_email)
                .await?
                .is_some()
        );
        Ok(())
    }
}
