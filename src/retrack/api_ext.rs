use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    retrack::tags::prepare_tags,
};
use anyhow::{Context, bail};
use retrack_types::trackers::{
    Page, Tracker, TrackerCreateParams, TrackerDataRevision, TrackerDataRevisionImportParams,
    TrackerDataRevisionImportResult, TrackerDebugParams, TrackerExecutionLog,
    TrackerListRevisionsParams, TrackerUpdateParams,
};
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

/// API to work with Retrack.
pub struct RetrackApi<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> RetrackApi<'a, DR, ET> {
    const LIST_TRACKERS_PAGE_SIZE: usize = 100;

    /// Creates Retrack API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Retrieves the Retrack trackers by the specified tags.
    pub async fn list_trackers<Tag: AsRef<str>>(
        &self,
        tags: &[Tag],
    ) -> anyhow::Result<Vec<Tracker>> {
        let prepared_tags = prepare_tags(tags);
        let tags_query = prepared_tags
            .iter()
            .map(|tag| format!("tag={}", urlencoding::encode(tag)))
            .collect::<Vec<_>>()
            .join("&");
        let mut trackers = vec![];
        let mut page_index = 0;

        loop {
            let page_query = format!(
                "page={page_index}&pageSize={}{}{}",
                Self::LIST_TRACKERS_PAGE_SIZE,
                if tags_query.is_empty() { "" } else { "&" },
                tags_query
            );
            let endpoint = format!("{}api/trackers?{page_query}", self.api.config.retrack.host);
            let response = self
                .api
                .network
                .http_client
                .get(&endpoint)
                .send()
                .await
                .with_context(|| format!("Cannot query trackers ({page_query})."))?;
            let mut page = response
                .json::<Page<Tracker>>()
                .await
                .context(format!("Cannot deserialize trackers ({page_query})."))?;

            let total = page.total.max(0) as usize;
            let is_last_page = page.items.is_empty() || trackers.len() + page.items.len() >= total;
            trackers.append(&mut page.items);
            if is_last_page {
                break;
            }
            page_index += 1;
        }

        Ok(trackers)
    }

    /// Retrieves Retrack trackers with the specified IDs.
    pub async fn bulk_get_trackers(&self, ids: &[Uuid]) -> anyhow::Result<Vec<Tracker>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let endpoint = format!("{}api/trackers/_bulk_get", self.api.config.retrack.host);
        self.api
            .network
            .http_client
            .post(&endpoint)
            .json(&serde_json::json!({ "ids": ids }))
            .send()
            .await
            .with_context(|| "Cannot bulk-retrieve trackers.")?
            .json()
            .await
            .context("Cannot deserialize bulk-retrieved trackers.")
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

    /// Removes all Retrack trackers that match the specified tags via the Retrack bulk-remove
    /// endpoint, returning the number of removed trackers. Tags are prepared exactly as in
    /// [`Self::list_trackers`] (prefixed with the `secutils` application tag), so removal is always
    /// scoped to Secutils trackers - it's the caller's responsibility to supply tags specific
    /// enough to target only the intended trackers. Passing an empty tags list therefore removes
    /// *all* Secutils trackers.
    pub async fn remove_trackers<Tag: AsRef<str>>(&self, tags: &[Tag]) -> anyhow::Result<u64> {
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
            .delete(&endpoint)
            .send()
            .await
            .with_context(|| format!("Cannot remove trackers ({tags_query})."))?;

        let status_code = response.status();
        if status_code.is_informational() || status_code.is_success() {
            return response.json().await.context(format!(
                "Cannot deserialize the number of removed trackers ({tags_query})."
            ));
        }

        let error_message = format!(
            "Failed to remove trackers ({tags_query}): {}",
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
            if params.calculate_diff {
                "true"
            } else {
                "false"
            }
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

    /// Retrieves data revisions for multiple trackers in a single batch request.
    pub async fn list_tracker_revisions_batch(
        &self,
        tracker_ids: &[Uuid],
        size: usize,
    ) -> anyhow::Result<HashMap<Uuid, Vec<TrackerDataRevision>>> {
        self.api
            .network
            .http_client
            .post(format!(
                "{}api/trackers/revisions",
                self.api.config.retrack.host
            ))
            .json(&serde_json::json!({
                "trackerIds": tracker_ids,
                "size": size,
            }))
            .send()
            .await
            .context("Cannot retrieve batch tracker data revisions.")?
            .json()
            .await
            .context("Cannot deserialize batch tracker data revisions.")
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

    /// Retrieves execution logs for a tracker.
    pub async fn list_tracker_execution_logs(
        &self,
        id: Uuid,
    ) -> anyhow::Result<Vec<TrackerExecutionLog>> {
        self.api
            .network
            .http_client
            .get(format!(
                "{}api/trackers/{id}/execution-logs",
                self.api.config.retrack.host
            ))
            .send()
            .await
            .with_context(|| format!("Cannot retrieve tracker execution logs ({id})."))?
            .json()
            .await
            .with_context(|| format!("Cannot deserialize tracker execution logs ({id})."))
    }

    /// Retrieves execution logs for multiple trackers in a single batch request.
    pub async fn list_tracker_execution_logs_batch(
        &self,
        tracker_ids: &[Uuid],
        size: usize,
    ) -> anyhow::Result<HashMap<Uuid, Vec<TrackerExecutionLog>>> {
        self.api
            .network
            .http_client
            .post(format!(
                "{}api/trackers/execution-logs",
                self.api.config.retrack.host
            ))
            .json(&serde_json::json!({
                "trackerIds": tracker_ids,
                "size": size,
            }))
            .send()
            .await
            .context("Cannot retrieve batch tracker execution logs.")?
            .json()
            .await
            .context("Cannot deserialize batch tracker execution logs.")
    }

    /// Clears execution logs for a tracker.
    pub async fn clear_tracker_execution_logs(&self, id: Uuid) -> anyhow::Result<()> {
        let response = self
            .api
            .network
            .http_client
            .delete(format!(
                "{}api/trackers/{id}/execution-logs",
                self.api.config.retrack.host
            ))
            .send()
            .await
            .with_context(|| format!("Cannot clear tracker execution logs ({id})."))?;

        let status_code = response.status();
        if status_code.is_informational() || status_code.is_success() {
            return Ok(());
        }

        let error_message = format!(
            "Failed to clear tracker execution logs ({id}): {}",
            response.text().await?
        );
        if status_code.is_client_error() {
            bail!(SecutilsError::client(error_message))
        } else {
            bail!(error_message)
        }
    }

    /// Bulk-imports historical revisions for a tracker.
    pub async fn import_tracker_revisions(
        &self,
        id: Uuid,
        revisions: &[TrackerDataRevisionImportParams],
    ) -> anyhow::Result<TrackerDataRevisionImportResult> {
        let response = self
            .api
            .network
            .http_client
            .post(format!(
                "{}api/trackers/{id}/revisions/_import",
                self.api.config.retrack.host
            ))
            .json(revisions)
            .send()
            .await
            .with_context(|| format!("Cannot import tracker revisions ({id})."))?;

        let status_code = response.status();
        if status_code.is_success() {
            return response.json().await.context(format!(
                "Cannot deserialize tracker revision import result ({id})."
            ));
        }

        let error_message = format!(
            "Failed to import tracker revisions ({id}): {}",
            response.text().await?
        );
        if status_code.is_client_error() {
            bail!(SecutilsError::client(error_message))
        } else {
            bail!(error_message)
        }
    }

    /// Runs the full tracker debug pipeline without persisting anything.
    pub async fn debug_tracker(&self, params: &TrackerDebugParams) -> anyhow::Result<JsonValue> {
        let response = self
            .api
            .network
            .http_client
            .post(format!(
                "{}api/trackers/_debug",
                self.api.config.retrack.host
            ))
            .json(params)
            .send()
            .await
            .context("Cannot execute tracker debug request.")?;

        let status_code = response.status();
        if status_code.is_success() {
            return response
                .json()
                .await
                .context("Cannot deserialize tracker debug result.");
        }

        let error_message = format!("Failed to debug tracker: {}", response.text().await?);
        if status_code.is_client_error() {
            bail!(SecutilsError::client(error_message))
        } else {
            bail!(error_message)
        }
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with Retrack.
    pub fn retrack(&self) -> RetrackApi<'_, DR, ET> {
        RetrackApi::new(self)
    }
}
