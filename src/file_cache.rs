use anyhow::Context;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct FileCache {}
impl FileCache {
    pub fn ensure_cache_dir_exists<P: AsRef<Path>>(relative_path: P) -> anyhow::Result<PathBuf> {
        let target_dir = Self::ensure_cache_root_exists()?.join(relative_path.as_ref());

        Self::ensure_dir_exists(&target_dir)?;

        Ok(target_dir)
    }

    pub fn ensure_dir_exists<P: AsRef<Path>>(absolute_path: P) -> anyhow::Result<()> {
        fs::create_dir_all(absolute_path.as_ref())
            .map_err(|err| {
                log::error!("Cannot create {:?} dir: {:?}", absolute_path.as_ref(), err);
                err
            })
            .with_context(|| format!("Cannot create {:?} dir.", absolute_path.as_ref()))
    }

    fn ensure_cache_root_exists() -> anyhow::Result<PathBuf> {
        dirs::cache_dir()
            .with_context(|| "Cache directory is not available.".to_string())
            .and_then(|cache_root| {
                let cache_dir = cache_root.join("secutils");

                fs::create_dir_all(&cache_dir)
                    .with_context(|| format!("Cannot create cache dir: {:?}.", cache_dir))?;

                Ok(cache_dir)
            })
    }
}
