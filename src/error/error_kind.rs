/// Describes a Secutils.dev specific error types.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// Error caused by the error on the client side.
    ClientError,
    /// Error caused by the lack of privileges to perform an action.
    AccessForbidden,
    /// Unknown error.
    Unknown,
}
