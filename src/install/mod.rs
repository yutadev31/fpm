use std::{fs::File, io::BufReader};

use tar::Archive;
use zstd::stream::read::Decoder;

use crate::{
    DIST_DIR,
    utils::{log::*, *},
};

async fn read_metadata(package_id: &str) -> anyhow::Result<(String, String, String)> {
    info!("Reading package metadata...");

    let metadata_path = format!("{}/{}.meta", DIST_DIR, package_id);
    let metadata = fs::read_file(&metadata_path).await?;

    let mut lines = metadata.lines();
    let name = lines.next().unwrap().to_string();
    let version = lines.next().unwrap().to_string();
    let release = lines.next().unwrap().to_string();

    info!("Package metadata read successfully!");
    Ok((name, version, release))
}

fn extract_package(filepath: &str) -> anyhow::Result<()> {
    info!("Extracting package...");

    let file = File::open(filepath)?;
    let buf = BufReader::new(file);

    let mut decoder = Decoder::new(buf)?;
    let mut tar = Archive::new(&mut decoder);

    for entry in tar.entries()? {
        let entry = entry?;
        let path = entry.path()?;

        println!("File: {}", path.to_string_lossy());
    }

    Ok(())
}

pub async fn install_packages(packages: &Vec<String>) -> anyhow::Result<()> {
    for package in packages {
        let (name, version, release) = read_metadata(package).await?;
        let filepath = format!("{}/{}-{}-{}.tar.zst", DIST_DIR, name, version, release);

        extract_package(&filepath)?;
    }

    Ok(())
}
