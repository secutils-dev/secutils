pub mod user_log_context;
mod utils_resource_log_context;

#[cfg(test)]
pub mod tests {
    pub use super::{
        user_log_context::UserLogContext, utils_resource_log_context::UtilsResourceLogContext,
    };
}
