use tantivy::schema::*;

#[derive(Copy, Clone)]
pub struct SearchIndexSchemaFields {
    pub id: Field,
    pub user_id: Field,
    pub label: Field,
    pub keywords: Field,
    pub category: Field,
    pub sub_category: Field,
    pub meta: Field,
    pub timestamp: Field,
}

impl SearchIndexSchemaFields {
    pub fn build() -> (Self, Schema) {
        let mut schema_builder = Schema::builder();
        (
            Self {
                id: schema_builder.add_u64_field("id", FAST | INDEXED | STORED),
                user_id: schema_builder.add_i64_field("user_id", FAST | INDEXED | STORED),
                label: schema_builder.add_text_field("label", TEXT | STORED),
                keywords: schema_builder.add_text_field(
                    "keywords",
                    TextOptions::default().set_stored().set_indexing_options(
                        TextFieldIndexing::default()
                            .set_tokenizer("en_stem")
                            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                    ),
                ),
                category: schema_builder.add_text_field(
                    "category",
                    TextOptions::default()
                        .set_fast()
                        .set_stored()
                        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("ids")),
                ),
                sub_category: schema_builder.add_text_field(
                    "sub_category",
                    TextOptions::default()
                        .set_fast()
                        .set_stored()
                        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("ids")),
                ),
                meta: schema_builder.add_bytes_field("meta", STORED),
                timestamp: schema_builder.add_date_field("timestamp", FAST | STORED),
            },
            schema_builder.build(),
        )
    }
}
