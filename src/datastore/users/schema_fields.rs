use tantivy::schema::*;

#[derive(Copy, Clone)]
pub struct UsersSchemaFields {
    pub email: Field,
    pub handle: Field,
    pub created: Field,
    pub password_hash: Field,
    pub roles: Field,
    pub profile: Field,
    pub activation_code: Field,
}

impl UsersSchemaFields {
    pub fn build() -> (Self, Schema) {
        let mut schema_builder = Schema::builder();
        (
            Self {
                email: schema_builder.add_text_field(
                    "email",
                    TextOptions::default()
                        .set_fast()
                        .set_stored()
                        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("ids")),
                ),
                handle: schema_builder.add_text_field(
                    "handle",
                    TextOptions::default()
                        .set_fast()
                        .set_stored()
                        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("ids")),
                ),
                password_hash: schema_builder.add_text_field("password_hash", STRING | STORED),
                roles: schema_builder.add_text_field("roles", STRING | STORED),
                profile: schema_builder.add_bytes_field("profile", STORED),
                created: schema_builder.add_date_field("created", FAST | INDEXED | STORED),
                activation_code: schema_builder.add_text_field(
                    "activation_code",
                    TextOptions::default()
                        .set_fast()
                        .set_stored()
                        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("ids")),
                ),
            },
            schema_builder.build(),
        )
    }
}
