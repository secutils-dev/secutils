mod users;

pub use self::users::{UsersIndex, UsersSchemaFields, UsersSearchFilter};
use crate::file_cache::FileCache;
use std::path::Path;
use tantivy::{
    directory::MmapDirectory,
    schema::*,
    tokenizer::{LowerCaser, RawTokenizer, TextAnalyzer},
    Index, IndexReader, IndexWriter, ReloadPolicy,
};
use time::OffsetDateTime;

#[derive(Clone)]
pub struct Datastore {
    pub users: UsersIndex,
}

impl Datastore {
    pub fn open<P: AsRef<Path>>(root_data_path: P) -> anyhow::Result<Self> {
        Ok(Self {
            users: UsersIndex::open(|schema| {
                open_index(root_data_path.as_ref().join("users"), schema)
            })?,
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
    FileCache::ensure_dir_exists(&index_path)?;

    let index_directory = MmapDirectory::open(&index_path)?;

    let index = if Index::exists(&index_directory)? {
        Index::open_in_dir(&index_path)?
    } else {
        Index::create_in_dir(&index_path, schema)?
    };

    initialize_index(index)
}

pub fn initialize_index(index: Index) -> anyhow::Result<(Index, IndexReader)> {
    index
        .tokenizers()
        .register("ids", TextAnalyzer::from(RawTokenizer).filter(LowerCaser));

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
