use tokio::process::Command;

use crate::{PKGS_DIR, SCRIPT_NAME, utils::log::info};

pub struct Package {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) release: String,
    pub(crate) sources: Vec<String>,
    pub(crate) sha256sums: Vec<String>,
    pub(crate) sha512sums: Vec<String>,
    pub(crate) b2sums: Vec<String>,
    pub(crate) validpgpkeys: Vec<String>,
    pub(crate) provides: Vec<String>,
    pub(crate) dependencies: Vec<String>,
}

impl Package {
    pub async fn get(package_name: &str, mirrors: Vec<(String, String)>) -> anyhow::Result<Self> {
        info!("Getting package information...");

        async fn get_package_var(
            package_name: &str,
            var_name: &str,
            is_list: bool,
        ) -> anyhow::Result<String> {
            let var = if is_list {
                format!("${{{}[@]}}", var_name)
            } else {
                format!("${{{}}}", var_name)
            };

            let output = Command::new("bash")
                .arg("-eu")
                .arg("-c")
                .arg(format!(
                    "source '{}/{}/{}'; echo \"{}\"",
                    PKGS_DIR, package_name, SCRIPT_NAME, var
                ))
                .output()
                .await?;

            if !output.status.success() {
                anyhow::bail!(
                    "Failed to get variable {} for package {}",
                    var_name,
                    package_name
                );
            }

            let var_value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(var_value)
        }

        let pkg_name = get_package_var(package_name, "name", false).await?;
        let pkg_version = get_package_var(package_name, "version", false).await?;
        let pkg_release = get_package_var(package_name, "release", false).await?;
        let pkg_sources = get_package_var(package_name, "sources", true).await?;
        let pkg_sha256sums = get_package_var(package_name, "sha256sums", true).await?;
        let pkg_sha512sums = get_package_var(package_name, "sha512sums", true).await?;
        let pkg_b2sums = get_package_var(package_name, "b2sums", true).await?;
        let pkg_validpgpkeys = get_package_var(package_name, "validpgpkeys", true).await?;
        let pkg_provides = get_package_var(package_name, "provides", true).await?;
        let pkg_dependencies = get_package_var(package_name, "dependencies", true).await?;

        fn parse_list(var: &str) -> Vec<String> {
            var.split_whitespace().map(|s| s.to_string()).collect()
        }

        fn replace_mirrors(sources: Vec<String>, mirrors: Vec<(String, String)>) -> Vec<String> {
            sources
                .iter()
                .map(|src| {
                    let mut src = src.clone();
                    for mirror in mirrors.iter() {
                        src = src.replace(&mirror.0, &mirror.1);
                    }
                    src
                })
                .collect()
        }

        let info = Self {
            name: pkg_name,
            version: pkg_version,
            release: pkg_release,
            sources: replace_mirrors(parse_list(&pkg_sources), mirrors),
            sha256sums: parse_list(&pkg_sha256sums),
            sha512sums: parse_list(&pkg_sha512sums),
            b2sums: parse_list(&pkg_b2sums),
            validpgpkeys: parse_list(&pkg_validpgpkeys),
            provides: parse_list(&pkg_provides),
            dependencies: parse_list(&pkg_dependencies),
        };

        println!(
            "Package information retrieved: {} {}",
            info.name, info.version
        );

        Ok(info)
    }
}
