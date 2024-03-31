use tantivy::schema::*;

#[derive(Copy, Clone)]
pub struct SearchIndexSchemaFields {
    pub id: Field,
    pub user_id: Field,
    pub label: Field,
    pub label_ngram: Field,
    pub keywords: Field,
    pub keywords_ngram: Field,
    pub category: Field,
    pub sub_category: Field,
    pub meta: Field,
    pub timestamp: Field,
}

fn ids_field_option() -> TextOptions {
    TextOptions::default()
        .set_fast(Some("ids"))
        .set_stored()
        .set_indexing_options(TextFieldIndexing::default().set_tokenizer("ids"))
}

impl SearchIndexSchemaFields {
    pub fn build() -> (Self, Schema) {
        let mut schema_builder = Schema::builder();
        (
            Self {
                id: schema_builder.add_u64_field("id", FAST | INDEXED | STORED),
                user_id: schema_builder.add_text_field("user_id", STRING | STORED),
                label: schema_builder.add_text_field("label", STRING | STORED),
                label_ngram: schema_builder.add_text_field(
                    "label_ngram",
                    TextOptions::default().set_stored().set_indexing_options(
                        TextFieldIndexing::default()
                            .set_tokenizer("ngram2_10")
                            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                    ),
                ),
                keywords: schema_builder.add_text_field("keywords", STRING | STORED),
                keywords_ngram: schema_builder.add_text_field(
                    "keywords_ngram",
                    TextOptions::default().set_stored().set_indexing_options(
                        TextFieldIndexing::default()
                            .set_tokenizer("ngram2_10")
                            .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                    ),
                ),
                category: schema_builder.add_text_field("category", ids_field_option()),
                sub_category: schema_builder.add_text_field("sub_category", ids_field_option()),
                meta: schema_builder.add_bytes_field("meta", STORED),
                timestamp: schema_builder.add_date_field("timestamp", FAST | STORED),
            },
            schema_builder.build(),
        )
    }
}
