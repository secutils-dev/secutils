use anyhow::{Context, anyhow};
use directories::ProjectDirs;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Directories;
impl Directories {
    pub fn ensure_data_dir_exists() -> anyhow::Result<PathBuf> {
        ProjectDirs::from("dev", "secutils.dev", "secutils")
            .ok_or_else(|| anyhow!("Project data directory is not available."))
            .and_then(|project_dirs| {
                let data_dir = project_dirs.data_dir();

                Self::ensure_dir_exists(data_dir)?;

                Ok(data_dir.to_path_buf())
            })
    }

    pub fn ensure_dir_exists<P: AsRef<Path>>(absolute_path: P) -> anyhow::Result<()> {
        fs::create_dir_all(absolute_path.as_ref())
            .map_err(|err| {
                log::error!("Cannot create {:?} dir: {:?}", absolute_path.as_ref(), err);
                err
            })
            .with_context(|| format!("Cannot create {:?} dir.", absolute_path.as_ref()))
    }
}
