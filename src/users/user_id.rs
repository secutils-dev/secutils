#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct UserId(pub i64);
impl UserId {
    pub const fn empty() -> Self {
        Self(-1)
    }
}

#[cfg(test)]
mod tests {
    use crate::users::UserId;

    #[test]
    fn empty() {
        assert_eq!(UserId::empty(), UserId(-1));
    }
}
