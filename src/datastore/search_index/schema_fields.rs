use tantivy::schema::*;

#[derive(Copy, Clone)]
pub struct SearchIndexSchemaFields {
    pub id: Field,
    pub user_handle: Field,
    pub timestamp: Field,
    pub content: Field,
}

impl SearchIndexSchemaFields {
    pub fn build() -> (Self, Schema) {
        let mut schema_builder = Schema::builder();
        (
            Self {
                id: schema_builder.add_text_field(
                    "id",
                    TextOptions::default()
                        .set_fast()
                        .set_stored()
                        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("ids")),
                ),
                user_handle: schema_builder.add_text_field(
                    "user_handle",
                    TextOptions::default()
                        .set_fast()
                        .set_stored()
                        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("ids")),
                ),
                timestamp: schema_builder.add_date_field("timestamp", FAST | INDEXED | STORED),
                content: schema_builder.add_text_field("content", TEXT | STORED),
            },
            schema_builder.build(),
        )
    }
}
