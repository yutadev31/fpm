pub(crate) mod common;
pub(crate) mod install;
pub(crate) mod make;
pub(crate) mod package;
pub(crate) mod remove;
pub(crate) mod script;
pub(crate) mod utils;

const PKGS_DIR: &str = "./pkgs";
const SCRIPT_NAME: &str = "make.sh";
const DIST_DIR: &str = "./.dist";
const SOURCES_DIR: &str = "./.sources";
const LOCAL_PKG_DB_PATH: &str = "/var/lib/kpm/local-db.csv";
const DOWNLOAD_WAIT_TIME: u64 = 500;
const MIRROR_FILE: &str = "mirrors.txt";

pub use install::install_packages;
pub use make::{MakeOptions, make_packages};
pub use remove::remove_packages;
pub use script::{new_build_script, update_build_script};

async fn get_all_build_scripts() -> anyhow::Result<Vec<String>> {
    let scripts = utils::fs::list_dir(PKGS_DIR).await?;
    Ok(scripts)
}
