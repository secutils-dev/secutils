use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::{SharedResource, User, UserId, UserShare, UserShareId},
};

pub mod errors;

pub struct UsersApi<'a, DR: DnsResolver, ET: EmailTransport> {
    api: &'a Api<DR, ET>,
}

impl<'a, DR: DnsResolver, ET: EmailTransport> UsersApi<'a, DR, ET> {
    /// Creates Users API.
    pub fn new(api: &'a Api<DR, ET>) -> Self {
        Self { api }
    }

    /// Retrieves the user by the specified ID.
    pub async fn get(&self, id: UserId) -> anyhow::Result<Option<User>> {
        self.api.db.get_user(id).await
    }

    /// Retrieves the user using the specified email.
    pub async fn get_by_email<E: AsRef<str>>(&self, user_email: E) -> anyhow::Result<Option<User>> {
        self.api.db.get_user_by_email(user_email).await
    }

    /// Retrieves the user using the specified handle.
    pub async fn get_by_handle<E: AsRef<str>>(
        &self,
        user_handle: E,
    ) -> anyhow::Result<Option<User>> {
        self.api.db.get_user_by_handle(user_handle).await
    }

    /// Retrieves the user share by the specified ID.
    pub async fn get_user_share(&self, id: UserShareId) -> anyhow::Result<Option<UserShare>> {
        self.api.db.get_user_share(id).await
    }

    /// Retrieves the user share by the specified user ID and resource.
    pub async fn get_user_share_by_resource(
        &self,
        user_id: UserId,
        resource: &SharedResource,
    ) -> anyhow::Result<Option<UserShare>> {
        self.api
            .db
            .get_user_share_by_resource(user_id, resource)
            .await
    }

    /// Inserts user share into the database.
    pub async fn insert_user_share(&self, user_share: &UserShare) -> anyhow::Result<()> {
        self.api.db.insert_user_share(user_share).await
    }

    /// Removes user share with the specified ID from the database.
    pub async fn remove_user_share(&self, id: UserShareId) -> anyhow::Result<Option<UserShare>> {
        self.api.db.remove_user_share(id).await
    }
}

impl<DR: DnsResolver, ET: EmailTransport> Api<DR, ET> {
    /// Returns an API to work with users.
    pub fn users(&self) -> UsersApi<'_, DR, ET> {
        UsersApi::new(self)
    }
}
