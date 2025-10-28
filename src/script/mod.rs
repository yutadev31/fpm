use crate::{SCRIPT_NAME, utils::fs};

pub async fn new_build_script(package: &str) -> anyhow::Result<()> {
    let dir_path = format!("pkgs/{}", package);
    let script_path = format!("{}/{}", dir_path, SCRIPT_NAME);

    if fs::exists_file(&script_path).await? {
        println!("Build script already exists for package {}", package);
    } else {
        fs::create_dir(&dir_path).await?;

        let content = format!(
            "name=\"{}\"\nversion=\"\"\nrelease=\"\"\nsources=()\n",
            package
        );

        fs::write_file(&script_path, &content).await?;
    }

    Ok(())
}

pub async fn update_build_script(package: &str, version: &str) -> anyhow::Result<()> {
    let script_path = format!("pkgs/{}/{}", package, SCRIPT_NAME);

    if !fs::exists_file(&script_path).await? {
        let content = fs::read_file(&script_path).await?;

        let regex = regex::Regex::new(r"^version=\*$").unwrap();
        let content = regex.replace(&content, format!("version=\"{}\"", version));

        let regex = regex::Regex::new(r"^release=\*$").unwrap();
        let content = regex.replace(&content, "release=\"1\"".to_string());

        fs::write_file(&script_path, &content).await?;
    }

    Ok(())
}
