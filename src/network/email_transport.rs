use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    transport::{
        smtp::Error as SmtpError,
        stub::{AsyncStubTransport, Error as StubError},
    },
};
use std::error::Error as StdError;

pub trait EmailTransport: AsyncTransport + Sync + Send + 'static {}
impl EmailTransport for AsyncSmtpTransport<Tokio1Executor> {}
impl EmailTransport for AsyncStubTransport {}

pub trait EmailTransportError: StdError + Sync + Send {}
impl EmailTransportError for SmtpError {}
impl EmailTransportError for StubError {}
