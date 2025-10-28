use std::path::Path;

use tokio::fs;

pub async fn create_dir(path: &str) -> anyhow::Result<()> {
    if !Path::new(path).is_dir() {
        fs::create_dir_all(path).await?;
    }
    Ok(())
}

pub async fn remove_dir(path: &str) -> anyhow::Result<()> {
    if Path::new(path).is_dir() {
        fs::remove_dir_all(path).await?;
    }
    Ok(())
}

pub async fn remove_file(path: &str) -> anyhow::Result<()> {
    if Path::new(path).exists() {
        fs::remove_file(path).await?;
    }
    Ok(())
}

pub async fn open_file(path: &str) -> anyhow::Result<fs::File> {
    let file = fs::File::open(path).await?;
    Ok(file)
}

pub async fn read_file(path: &str) -> anyhow::Result<String> {
    let content = fs::read_to_string(path).await?;
    Ok(content)
}

pub async fn write_file(path: &str, content: &str) -> anyhow::Result<()> {
    fs::write(path, content).await?;
    Ok(())
}

pub async fn create_file(path: &str) -> anyhow::Result<()> {
    if !Path::new(path).exists() {
        write_file(path, "").await?;
    }
    Ok(())
}

pub async fn list_dir(dir: &str) -> anyhow::Result<Vec<String>> {
    let mut entries = fs::read_dir(dir).await?;
    let mut files = Vec::new();

    while let Some(entry) = entries.next_entry().await? {
        files.push(entry.file_name().into_string().unwrap());
    }

    Ok(files)
}

pub async fn exists_file(path: &str) -> anyhow::Result<bool> {
    Ok(Path::new(path).exists())
}

pub async fn copy_file(src: &str, dest: &str) -> anyhow::Result<()> {
    fs::copy(src, dest).await?;
    Ok(())
}
