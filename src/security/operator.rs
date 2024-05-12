/// Struct to represent an operator account.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Operator(String);
impl Operator {
    /// Creates a new operator account with the provided ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the ID of the operator account.
    pub fn id(&self) -> &str {
        &self.0
    }
}
