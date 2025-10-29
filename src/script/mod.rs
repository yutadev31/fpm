use std::collections::HashMap;

use crate::{
    SCRIPT_NAME,
    package::Package,
    utils::{
        fs,
        log::{error, info, warning},
    },
};

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

pub async fn check_dependencies() -> anyhow::Result<()> {
    let dirname_list = fs::list_dir("pkgs").await?;
    let mut packages = Vec::new();

    for dirname in dirname_list {
        let info = Package::get(&dirname, Vec::default()).await?;
        packages.push(info);
    }

    let name_map: HashMap<_, _> = packages.iter().map(|p| (p.name.as_str(), p)).collect();

    // provides名 -> Package
    let mut provides_map: HashMap<&str, &Package> = HashMap::new();
    for pkg in &packages {
        for p in &pkg.provides {
            provides_map.insert(p.as_str(), pkg);
        }
    }

    // 依存解決
    for pkg in &packages {
        info!("Checking {} dependencies...", pkg.name);

        if pkg.dependencies.is_empty() {
            warning!("{} has no dependencies", pkg.name);
        }

        for dep in &pkg.dependencies {
            if let Some(provider) = name_map.get(dep.as_str()) {
                println!("{} depends on {} (package)", pkg.name, provider.name);
            } else if let Some(provider) = provides_map.get(dep.as_str()) {
                println!(
                    "{} depends on {} (provided by {})",
                    pkg.name, dep, provider.name
                );
            } else {
                error!("{} depends on missing dependency: {}", pkg.name, dep);
            }
        }
    }

    Ok(())
}
