use crate::users::UserId;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchFilter<'q, 'c> {
    pub user_id: Option<UserId>,
    pub query: Option<&'q str>,
    pub category: Option<&'c str>,
}

impl<'q, 'c> SearchFilter<'q, 'c> {
    pub fn with_user_id(self, user_id: UserId) -> Self {
        Self {
            user_id: Some(user_id),
            ..self
        }
    }

    pub fn with_query(self, query: &'q str) -> Self {
        Self {
            query: Some(query),
            ..self
        }
    }

    pub fn with_category(self, category: &'c str) -> Self {
        Self {
            category: Some(category),
            ..self
        }
    }
}
