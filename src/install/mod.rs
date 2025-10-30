use std::{
    fs::{self, File},
    io::{self, BufReader, Write},
    os::unix,
    path::{Path, PathBuf},
};

use anyhow::bail;
use tar::Archive;
use zstd::stream::read::Decoder;

use crate::utils::log::*;

pub async fn install_packages(packages: &Vec<String>, dest: Option<String>) -> anyhow::Result<()> {
    let dest = dest.unwrap_or("/".to_string());

    let mut conflict_flag = false;
    let mut tar_archives = Vec::new();
    let mut file_lists = Vec::new();

    for package in packages {
        let filepath = package;
        let mut file_list = Vec::new();

        let file = File::open(filepath)?;
        let buf = BufReader::new(file);

        let decoder = Decoder::new(buf)?;
        let mut tar = Archive::new(decoder);

        for entry in tar.entries()? {
            let entry = entry?;
            let path = entry.path()?;

            let entry_type = entry.header().entry_type();
            if entry_type.is_file() {
                let dest_path = Path::new(&dest).join(&path);
                if dest_path.exists() {
                    error!("File already exists: {}", dest_path.display());
                    conflict_flag = true;
                }

                let path = PathBuf::from(path);
                file_list.push(path);
            } else if entry_type.is_symlink() {
                let path = PathBuf::from(path);
                file_list.push(path);
            }
        }

        tar_archives.push(tar);
        file_lists.push(file_list);
    }

    if conflict_flag {
        bail!("Conflicts detected")
    }

    let local_pkgs_dir = Path::new(&dest).join("var/lib/fpm/local/pkgs");

    for (index, package) in packages.iter().enumerate() {
        let db_path = local_pkgs_dir.join(format!("{}.txt", package));
        let mut db_file = File::create(db_path)?;

        for file in file_lists[index].iter() {
            writeln!(db_file, "{}", file.display())?;
        }

        for entry in tar_archives[index].entries()? {
            let mut entry = entry?;
            let path = entry.path()?;

            let dest_path = Path::new(&dest).join(path);
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
    }

    Ok(())
}
