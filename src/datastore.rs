mod primary_db;
mod search_index;

pub use self::{
    primary_db::PrimaryDb,
    search_index::{SearchFilter, SearchIndex, SearchIndexSchemaFields},
};
use crate::{config::Config, directories::Directories};
use std::path::Path;
use tantivy::{
    directory::MmapDirectory,
    schema::*,
    tokenizer::{LowerCaser, NgramTokenizer, RawTokenizer, TextAnalyzer},
    Index, IndexReader, IndexWriter, ReloadPolicy,
};
use time::OffsetDateTime;

#[derive(Clone)]
pub struct Datastore {
    pub primary_db: PrimaryDb,
    pub search_index: SearchIndex,
}

impl Datastore {
    pub async fn open<P: AsRef<Path>>(config: &Config, root_data_path: P) -> anyhow::Result<Self> {
        Ok(Self {
            search_index: SearchIndex::open(|schema| {
                open_index(
                    root_data_path.as_ref().join(format!(
                        "search_index_v{}",
                        config.components.search_index_version
                    )),
                    schema,
                )
            })?,
            primary_db: PrimaryDb::open(|| {
                root_data_path
                    .as_ref()
                    .to_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Cannot stringify database folder {:?}",
                            root_data_path.as_ref()
                        )
                    })
                    .map(|db_dir| format!("sqlite:{db_dir}/data.db?mode=rwc"))
            })
            .await?,
        })
    }
}

impl AsRef<Datastore> for Datastore {
    fn as_ref(&self) -> &Self {
        self
    }
}

pub fn open_index<P: AsRef<Path>>(
    index_path: P,
    schema: Schema,
) -> anyhow::Result<(Index, IndexReader)> {
    Directories::ensure_dir_exists(&index_path)?;

    let index_directory = MmapDirectory::open(&index_path)?;

    let index = if Index::exists(&index_directory)? {
        Index::open_in_dir(&index_path)?
    } else {
        log::warn!(
            "Search index data folder doesn't exist and will be created: {:?}.",
            index_path.as_ref()
        );
        Index::create_in_dir(&index_path, schema)?
    };

    initialize_index(index)
}

pub fn initialize_index(index: Index) -> anyhow::Result<(Index, IndexReader)> {
    let tokenizers = index.tokenizers();
    tokenizers.register("ngram2_10", NgramTokenizer::prefix_only(2, 10));

    let tokenizers = index.fast_field_tokenizer();
    tokenizers.register(
        "ids",
        TextAnalyzer::builder(RawTokenizer::default())
            .filter(LowerCaser)
            .build(),
    );

    let index_reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()?;

    Ok((index, index_reader))
}

pub fn commit_index(
    index_writer: &mut IndexWriter,
    index_reader: &IndexReader,
) -> anyhow::Result<()> {
    Ok(index_writer
        .prepare_commit()
        .and_then(|mut prepared_commit| {
            prepared_commit
                .set_payload(&OffsetDateTime::now_utc().unix_timestamp_nanos().to_string());
            prepared_commit.commit()?;
            index_reader.reload()
        })?)
}
