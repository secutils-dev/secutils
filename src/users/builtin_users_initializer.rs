use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::BuiltinUser,
};

pub async fn builtin_users_initializer<BU: AsRef<str>, DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    builtin_users: BU,
) -> anyhow::Result<()> {
    log::info!("Initializing builtin users");
    let users = api.users();

    let mut initialized_builtin_users = 0;
    for builtin_user_str in builtin_users.as_ref().split('|') {
        users
            .upsert_builtin(BuiltinUser::try_from(builtin_user_str)?)
            .await?;
        initialized_builtin_users += 1;
    }

    log::info!(
        "Successfully initialized {} builtin users.",
        initialized_builtin_users
    );

    Ok(())
}
