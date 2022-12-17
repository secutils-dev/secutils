mod auto_responder;
mod auto_responder_method;

pub use self::{auto_responder::AutoResponder, auto_responder_method::AutoResponderMethod};

pub const USER_PROFILE_DATA_KEY_AUTO_RESPONDERS: &str = "hp.ar";
