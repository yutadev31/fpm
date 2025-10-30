use std::{
    fs::{self, File},
    io::{self, BufReader},
    os::unix,
    path::Path,
};

use anyhow::bail;
use tar::Archive;
use zstd::stream::read::Decoder;

use crate::utils::log::*;

fn extract_package(filepath: &str, dest: &str) -> anyhow::Result<()> {
    info!("Extracting package...");

    let file = File::open(filepath)?;
    let buf = BufReader::new(file);

    let mut decoder = Decoder::new(buf)?;
    let mut tar = Archive::new(&mut decoder);

    let mut conflict_flag = false;
    for entry in tar.entries()? {
        let entry = entry?;
        let path = entry.path()?;

        let entry_type = entry.header().entry_type();
        if entry_type.is_file() {
            let dest_path = Path::new(dest).join(path);
            if dest_path.exists() {
                error!("File already exists: {}", dest_path.display());
                conflict_flag = true;
            }
        }
    }

    if conflict_flag {
        bail!("Conflicts detected")
    }

    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        let dest_path = Path::new(dest).join(path);
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            fs::create_dir_all(dest_path)?;
        } else if entry_type.is_file() {
            let mut outfile = File::create(dest_path)?;
            io::copy(&mut entry, &mut outfile)?;
        } else if entry_type.is_symlink()
            && let Some(target) = entry.link_name()?
        {
            unix::fs::symlink(target, dest_path)?;
        }
    }

    Ok(())
}

pub async fn install_packages(packages: &Vec<String>, dest: Option<String>) -> anyhow::Result<()> {
    let dest = dest.unwrap_or("/".to_string());

    for package in packages {
        extract_package(package, &dest)?;
    }

    Ok(())
}
