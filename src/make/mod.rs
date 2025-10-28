use std::{path::Path, process::exit, time::Duration};

use anyhow::anyhow;
use clap::Parser;
use regex::Regex;
use tokio::{process::Command, time::sleep};

use crate::{
    DIST_DIR, DOWNLOAD_WAIT_TIME, LOCAL_PKG_DB_PATH, MIRROR_FILE, PKGS_DIR, SCRIPT_NAME,
    SOURCES_DIR,
    common::{shell::run_shell, *},
    get_all_build_scripts,
    package::PkgInfo,
    utils::{
        log::{error, info},
        path::get_filename,
        *,
    },
};

#[derive(Debug, Clone, Parser)]
pub struct MakeOptions {
    #[arg()]
    package_name: Vec<String>,

    #[arg(long)]
    all: bool,

    #[arg(long)]
    rebuild: bool,

    #[arg(long)]
    disable_sign: bool,

    #[arg(long)]
    disable_check_depends: bool,
}

async fn run_script_function(
    description: &str,
    package_name: &str,
    function_name: &str,
    fakeroot: bool,
) -> anyhow::Result<()> {
    let (work_dir, pkg_dir) = get_dirs(package_name)?;

    let script = format!(
        "touch '.env.sh' && source '.env.sh'; source '{}/{}/{}'; export pkgdir='{}'; cd {}; if declare -F {} >/dev/null; then {}; fi",
        PKGS_DIR, package_name, SCRIPT_NAME, pkg_dir, work_dir, function_name, function_name
    );

    run_shell(description, &script, fakeroot).await?;

    Ok(())
}

fn get_dirs(package_name: &str) -> anyhow::Result<(String, String)> {
    let work_dir = format!("/tmp/work-{}", package_name);
    let pkg_dir = format!("/tmp/pkg-{}", package_name);

    Ok((work_dir, pkg_dir))
}

async fn create_tmp_dirs(package_name: &str) -> anyhow::Result<()> {
    let (work_dir, pkg_dir) = get_dirs(package_name)?;

    fs::create_dir(&work_dir).await?;
    fs::create_dir(&pkg_dir).await?;

    Ok(())
}

fn get_dist_filepath(info: &PkgInfo) -> String {
    format!(
        "{}/{}-{}-{}.tar.zstd",
        DIST_DIR, info.name, info.version, info.release
    )
}

async fn is_exists_dist(info: &PkgInfo) -> anyhow::Result<bool> {
    let filepath = get_dist_filepath(info);
    let exists = fs::exists_file(&filepath).await?;
    Ok(exists)
}

async fn get_mirrors(filepath: &str) -> anyhow::Result<Vec<(String, String)>> {
    let content = fs::read_file(filepath).await?;
    let lines = content.split('\n').filter(|line| !line.is_empty());
    let mirrors = lines
        .map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid mirror format")
            } else {
                Ok((parts[0].to_string(), parts[1].to_string()))
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(mirrors)
}

async fn get_pkg_info(
    package_name: &str,
    mirrors: Vec<(String, String)>,
) -> anyhow::Result<PkgInfo> {
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

    let info = PkgInfo {
        name: pkg_name,
        version: pkg_version,
        release: pkg_release,
        sources: replace_mirrors(parse_list(&pkg_sources), mirrors),
        sha256sums: parse_list(&pkg_sha256sums),
        sha512sums: parse_list(&pkg_sha512sums),
        b2sums: parse_list(&pkg_b2sums),
        validpgpkeys: parse_list(&pkg_validpgpkeys),
        dependencies: parse_list(&pkg_dependencies),
    };

    println!(
        "Package information retrieved: {} {}",
        info.name, info.version
    );

    Ok(info)
}

async fn check_dependencies(dependencies: &[String]) -> anyhow::Result<()> {
    info!("Checking dependencies...");

    fs::create_file(LOCAL_PKG_DB_PATH).await?;

    let local_db = fs::read_file(LOCAL_PKG_DB_PATH).await?;

    for dep in dependencies {
        let mut found = false;
        for line in local_db.lines() {
            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() >= 2 && fields[0] == dep {
                found = true;
                break;
            }
        }

        if !found {
            anyhow::bail!("Dependency {} is not installed.", dep);
        }
    }

    Ok(())
}

async fn download_sources(sources: &[String]) -> anyhow::Result<Vec<String>> {
    info!("Downloading sources...");

    let mut filepaths = Vec::new();
    let regex = Regex::new(r"http.*")?;

    for source in sources {
        if !regex.is_match(source) {
            continue;
        }

        let filename = get_filename(source).ok_or(anyhow!("Invalid URL: {}", source))?;
        let filepath = format!("{}/{}", SOURCES_DIR, filename);

        if Path::new(&filepath).is_file() {
            println!("Source already downloaded: {}", source);
            filepaths.push(filepath);
            continue;
        }

        println!("DEBUG: {}", source);

        http::download_file(source, &filepath).await?;
        sleep(Duration::from_millis(DOWNLOAD_WAIT_TIME)).await;

        if !Path::new(&filepath).is_file() {
            anyhow::bail!("Downloaded file not found: {}", filename);
        }

        filepaths.push(filepath);
    }

    Ok(filepaths)
}

async fn check_sums(filepaths: &[String], info: &PkgInfo) -> anyhow::Result<()> {
    info!("Checking checksums...");

    for (i, filepath) in filepaths.iter().enumerate() {
        if i < info.sha256sums.len() && !info.sha256sums[i].is_empty() {
            hash::check_sum(filepath, &info.sha256sums[i], hash::Algorithm::Sha256).await?;
        }
        if i < info.sha512sums.len() && !info.sha512sums[i].is_empty() {
            hash::check_sum(filepath, &info.sha256sums[i], hash::Algorithm::Sha512).await?;
        }
        if i < info.b2sums.len() && !info.b2sums[i].is_empty() {
            hash::check_sum(filepath, &info.sha256sums[i], hash::Algorithm::Blake2).await?;
        }
    }

    Ok(())
}

async fn verify_signatures(filepaths: &[String], _validpgpkeys: &[String]) -> anyhow::Result<()> {
    info!("Verifying signatures...");

    let mut sign_files = Vec::new();
    let mut source_files = Vec::new();

    for filepath in filepaths {
        if path::is_signature_file(filepath) {
            sign_files.push(filepath);
        } else {
            source_files.push(filepath);
        }
    }

    for sign_file in sign_files {
        let base_name = Path::new(sign_file)
            .file_stem()
            .ok_or(anyhow::anyhow!("Failed to get file stem"))?
            .to_string_lossy()
            .to_string();

        if let Some(source_file) = source_files
            .iter()
            .find(|f| {
                path::get_filename(f).is_some() && path::get_filename(f).unwrap() == base_name
            })
            .or(source_files.iter().find(|f| {
                path::get_filename(f).is_some() && path::get_filename(f).unwrap() == base_name
            }))
        {
            println!("Verifying signature for file: {}", source_file);

            let output = Command::new("gpg")
                .arg("--verify")
                .arg(sign_file)
                .arg(source_file)
                .output()
                .await?;

            if !output.status.success() {
                anyhow::bail!("Signature verification failed for file: {}", source_file);
            }

            // TODO validpgpkeysを使うように変更

            info!("Signature verified for file: {}", source_file);
        } else {
            anyhow::bail!(
                "No matching source file found for signature file: {}",
                sign_file
            );
        }
    }

    Ok(())
}

async fn extract_sources(info: &PkgInfo) -> anyhow::Result<()> {
    info!("Extracting sources...");
    let (work_dir, _) = get_dirs(&info.name)?;
    let regex = Regex::new(r"http.*")?;

    for source in &info.sources {
        let filename = get_filename(source).ok_or(anyhow::anyhow!("Failed to get file name"))?;
        let filepath = format!("{}/{}", SOURCES_DIR, filename);

        if regex.is_match(source) {
            if path::is_tar_file(&filename) {
                let script = format!("tar -xf {} -C {} --strip-components=1", filepath, work_dir);
                run_shell("Extracting tar file", &script, false).await?;
            } else {
                let dest_path = format!("{}/{}", work_dir, filename);
                fs::copy_file(&filepath, &dest_path).await?;
            }
        } else {
            let src_path = format!("pkgs/{}/{}", info.name, filename);
            let dest_path = format!("{}/{}", work_dir, filename);
            fs::copy_file(&src_path, &dest_path).await?;
        }
    }

    Ok(())
}

async fn prepare(info: &PkgInfo) -> anyhow::Result<()> {
    info!("Preparing...");
    run_script_function("Preparing", &info.name, "prepare", false).await?;
    Ok(())
}

async fn build(info: &PkgInfo) -> anyhow::Result<()> {
    info!("Building...");
    run_script_function("Building", &info.name, "build", false).await?;
    Ok(())
}

async fn test(info: &PkgInfo) -> anyhow::Result<()> {
    info!("Testing...");
    run_script_function("Testing", &info.name, "test", false).await?;
    Ok(())
}

async fn package(info: &PkgInfo) -> anyhow::Result<()> {
    info!("Packaging...");
    run_script_function("Packaging", &info.name, "package", true).await?;
    Ok(())
}

async fn create_archive(info: &PkgInfo) -> anyhow::Result<()> {
    info!("Creating archive...");

    let (_, pkg_dir) = get_dirs(&info.name)?;
    let output_name = format!("{}-{}-{}", info.name, info.version, info.release);

    fs::create_dir(DIST_DIR).await?;

    let script = format!(
        "tar -Izstd -cf '{}/{}.tar.zst' -C '{}' .",
        DIST_DIR, output_name, pkg_dir
    );

    run_shell("Creating archive", &script, true).await?;

    Ok(())
}

async fn sign_archive(info: &PkgInfo) -> anyhow::Result<()> {
    info!("Signing archive...");

    let output_name = format!("{}-{}-{}", info.name, info.version, info.release);

    let script = format!(
        "gpg --detach-sign --armor --output '{}/{}.tar.zst.asc' '{}/{}.tar.zst'",
        DIST_DIR, output_name, DIST_DIR, output_name
    );

    run_shell("Signing archive", &script, true).await?;

    Ok(())
}

async fn write_metadata(info: &PkgInfo) -> anyhow::Result<()> {
    info!("Writing package metadata...");

    let filepath = format!("{}/{}.meta", DIST_DIR, info.name);
    let content = format!("{}\n{}\n{}\n", info.name, info.version, info.release);

    fs::write_file(&filepath, &content).await?;

    Ok(())
}

async fn cleanup(info: &PkgInfo) -> anyhow::Result<()> {
    info!("Cleaning up...");

    let (work_dir, pkg_dir) = get_dirs(&info.name)?;

    fs::remove_dir(&work_dir).await?;
    fs::remove_dir(&pkg_dir).await?;

    Ok(())
}

async fn make_package(
    package_name: &str,
    rebuild: bool,
    disable_sign: bool,
    disable_check_depends: bool,
) {
    info!("Making \"{}\" package...", package_name);

    if user::is_user_root() {
        error!("Cannot build package as root");
        exit(1);
    }

    async fn inner(
        package_name: &str,
        rebuild: bool,
        disable_sign: bool,
        disable_check_depends: bool,
    ) -> anyhow::Result<()> {
        let mirrors = get_mirrors(MIRROR_FILE).await?;
        let info = get_pkg_info(package_name, mirrors).await?;

        if is_exists_dist(&info).await? {
            if rebuild {
                let filepath = get_dist_filepath(&info);
                fs::remove_file(&filepath).await?;
            } else {
                error!("Package already exists");
                exit(1);
            }
        }

        cleanup(&info).await?;

        if !disable_check_depends {
            check_dependencies(&info.dependencies).await?;
        }

        let filepaths = download_sources(&info.sources).await?;
        check_sums(&filepaths, &info).await?;
        verify_signatures(&filepaths, &info.validpgpkeys).await?;

        create_tmp_dirs(&info.name).await?;
        extract_sources(&info).await?;

        prepare(&info).await?;
        build(&info).await?;
        test(&info).await?;
        package(&info).await?;
        create_archive(&info).await?;

        if !disable_sign {
            sign_archive(&info).await?;
        }

        write_metadata(&info).await?;
        cleanup(&info).await?;

        Ok(())
    }

    match inner(package_name, rebuild, disable_sign, disable_check_depends).await {
        Ok(()) => info!("All done!"),
        Err(e) => error!("{}", e),
    }
}

pub async fn make_packages(opts: MakeOptions) -> anyhow::Result<()> {
    if opts.all {
        let packages = get_all_build_scripts().await?;
        for package in packages {
            make_package(
                &package,
                opts.rebuild,
                opts.disable_sign,
                opts.disable_check_depends,
            )
            .await;
        }
    } else {
        for package in opts.package_name {
            make_package(
                &package,
                opts.rebuild,
                opts.disable_sign,
                opts.disable_check_depends,
            )
            .await;
        }
    }
    Ok(())
}
