use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    retrack::tags::prepare_tags,
};
use anyhow::{Context, bail};
use retrack_types::trackers::{
    Tracker, TrackerCreateParams, TrackerDataRevision, TrackerListRevisionsParams,
    TrackerUpdateParams,
};
use serde::de::DeserializeOwned;
use uuid::Uuid;

/// API to work with Retrack.
pub struct RetrackApi<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> RetrackApi<'a, DR, ET> {
    /// Creates Retrack API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Retrieves the Retrack trackers by the specified tags.
    pub async fn list_trackers<Tag: AsRef<str>>(
        &self,
        tags: &[Tag],
    ) -> anyhow::Result<Vec<Tracker>> {
        // Construct tags query string.
        let tags_query = prepare_tags(tags)
            .iter()
            .map(|tag| format!("tag={}", urlencoding::encode(tag)))
            .collect::<Vec<_>>()
            .join("&");
        let endpoint = format!("{}api/trackers?{tags_query}", self.api.config.retrack.host);

        let response = self
            .api
            .network
            .http_client
            .get(&endpoint)
            .send()
            .await
            .with_context(|| format!("Cannot query trackers ({tags_query})."))?;
        response
            .json()
            .await
            .context(format!("Cannot deserialize trackers ({tags_query})."))
    }

    /// Retrieves the Retrack tracker with the specified ID.
    pub async fn get_tracker(&self, id: Uuid) -> anyhow::Result<Option<Tracker>> {
        let response = self
            .api
            .network
            .http_client
            .get(format!("{}api/trackers/{id}", self.api.config.retrack.host))
            .send()
            .await
            .with_context(|| format!("Cannot retrieve tracker ({id})."))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        response
            .json()
            .await
            .context(format!("Cannot deserialize tracker ({id})."))
    }

    /// Creates a new Retrack tracker.
    pub async fn create_tracker(&self, params: &TrackerCreateParams) -> anyhow::Result<Tracker> {
        let response = self
            .api
            .network
            .http_client
            .post(format!("{}api/trackers", self.api.config.retrack.host))
            .json(params)
            .send()
            .await
            .with_context(|| format!("Cannot create a tracker ({}).", params.name))?;

        let status_code = response.status();
        if status_code.is_success() {
            return response.json().await.context(format!(
                "Cannot deserialize a created tracker ({}).",
                params.name
            ));
        }

        let error_message = format!(
            "Failed to create a tracker ({}): {}",
            params.name,
            response.text().await?
        );
        if status_code.is_client_error() {
            bail!(SecutilsError::client(error_message))
        } else {
            bail!(error_message)
        }
    }

    /// Updates the Retrack tracker with the specified ID.
    pub async fn update_tracker(
        &self,
        id: Uuid,
        params: &TrackerUpdateParams,
    ) -> anyhow::Result<Tracker> {
        let response = self
            .api
            .network
            .http_client
            .put(format!("{}api/trackers/{id}", self.api.config.retrack.host))
            .json(params)
            .send()
            .await
            .context(format!("Cannot update a tracker ({id})."))?;

        let status_code = response.status();
        if status_code.is_success() {
            return response
                .json()
                .await
                .context(format!("Cannot deserialize an updated tracker ({id})."));
        }

        let error_message = format!(
            "Failed to update a tracker ({id}): {}",
            response.text().await?
        );
        if status_code.is_client_error() {
            bail!(SecutilsError::client(error_message))
        } else {
            bail!(error_message)
        }
    }

    /// Removes the Retrack tracker with the specified ID.
    pub async fn remove_tracker(&self, id: Uuid) -> anyhow::Result<()> {
        let response = self
            .api
            .network
            .http_client
            .delete(format!("{}api/trackers/{id}", self.api.config.retrack.host))
            .send()
            .await
            .with_context(|| format!("Cannot delete a tracker ({id})."))?;

        let status_code = response.status();
        if status_code.is_informational() || status_code.is_success() {
            return Ok(());
        }

        let error_message = format!(
            "Failed to delete tracker ({id}): {}",
            response.text().await?
        );
        if status_code.is_client_error() {
            bail!(SecutilsError::client(error_message))
        } else {
            bail!(error_message)
        }
    }

    /// Retrieves the Retrack tracker revisions by the specified ID.
    pub async fn list_tracker_revisions<TValue: DeserializeOwned>(
        &self,
        id: Uuid,
        params: TrackerListRevisionsParams,
    ) -> anyhow::Result<Vec<TrackerDataRevision<TValue>>> {
        let mut query_pairs = vec![format!(
            "calculateDiff={}",
            if params.calculate_diff { "true" } else { "false" }
        )];
        if let Some(context_radius) = params.context_radius {
            query_pairs.push(format!("contextRadius={context_radius}"));
        }
        if let Some(size) = params.size {
            query_pairs.push(format!("size={size}"));
        }

        self.api
            .network
            .http_client
            .get(format!(
                "{}api/trackers/{id}/revisions?{}",
                self.api.config.retrack.host,
                query_pairs.join("&")
            ))
            .send()
            .await
            .with_context(|| format!("Cannot retrieve tracker revisions ({id})."))?
            .json()
            .await
            .with_context(|| format!("Cannot deserialize tracker revisions ({id})."))
    }

    /// Clears the Retrack tracker revisions by the specified ID.
    pub async fn clear_tracker_revisions(&self, id: Uuid) -> anyhow::Result<()> {
        let response = self
            .api
            .network
            .http_client
            .delete(format!(
                "{}api/trackers/{id}/revisions",
                self.api.config.retrack.host
            ))
            .send()
            .await
            .with_context(|| format!("Cannot clear tracker revisions ({id})."))?;

        let status_code = response.status();
        if status_code.is_informational() || status_code.is_success() {
            return Ok(());
        }

        let error_message = format!(
            "Failed to clear tracker revisions ({id}): {}",
            response.text().await?
        );
        if status_code.is_client_error() {
            bail!(SecutilsError::client(error_message))
        } else {
            bail!(error_message)
        }
    }

    /// Creates a new revision for the tracker with the specified ID.
    pub async fn create_revision<TValue: DeserializeOwned>(
        &self,
        id: Uuid,
    ) -> anyhow::Result<TrackerDataRevision<TValue>> {
        let response = self
            .api
            .network
            .http_client
            .post(format!(
                "{}api/trackers/{id}/revisions",
                self.api.config.retrack.host
            ))
            .send()
            .await
            .with_context(|| format!("Cannot execute new tracker revision request ({id})."))?;

        let status_code = response.status();
        if status_code.is_success() {
            return response.json().await.context(format!(
                "Cannot deserialize a created tracker revision ({id})."
            ));
        }

        if status_code.is_client_error() {
            bail!(SecutilsError::client(response.text().await?))
        } else {
            bail!(response.text().await?)
        }
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with Retrack.
    pub fn retrack(&self) -> RetrackApi<'_, DR, ET> {
        RetrackApi::new(self)
    }
}
