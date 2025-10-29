use std::{path::Path, process::exit, time::Duration};

use anyhow::anyhow;
use clap::Parser;
use regex::Regex;
use tokio::time::sleep;

use crate::{
    DIST_DIR, DOWNLOAD_WAIT_TIME, LOCAL_PKG_DB_PATH, MIRROR_FILE, PKGS_DIR, SCRIPT_NAME,
    SOURCES_DIR,
    common::{shell::run_shell, *},
    get_all_build_scripts,
    package::Package,
    utils::{
        log::{error, info},
        path::get_filename,
        *,
    },
};

// struct SimpleVerifier {
//     cert: Vec<Cert>,
// }

// impl<'a> VerificationHelper for SimpleVerifier {
//     fn get_certs(&mut self, _ids: &[KeyHandle]) -> openpgp::Result<Vec<Cert>> {
//         // 署名者の鍵として渡す
//         Ok(self.cert.clone())
//     }

//     fn check(&mut self, structure: MessageStructure) -> anyhow::Result<()> {
//         for layer in structure.iter() {
//             if let MessageLayer::SignatureGroup { results } = layer {
//                 for sig in results {
//                     match sig {
//                         Ok(good) => {
//                             let fp_list = good.sig.issuer_fingerprints();
//                             for fp in fp_list {
//                                 let Some(_) =
//                                     self.cert.iter().find(|cert| cert.fingerprint() == *fp)
//                                 else {
//                                     bail!("Unknown certificate: {}", fp);
//                                 };
//                             }
//                         }
//                         Err(err) => {
//                             bail!("Verification failed: {}", err)
//                         }
//                     }
//                 }
//             }
//         }
//         Ok(())
//     }
// }

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
    package_name: &str,
    function_name: &str,
    fakeroot: bool,
) -> anyhow::Result<()> {
    let (work_dir, pkg_dir) = get_dirs(package_name)?;

    let script = format!(
        "touch '.env.sh' && source '.env.sh'; source '{}/{}/{}'; export pkgdir='{}'; cd {}; if declare -F {} >/dev/null; then {}; fi",
        PKGS_DIR, package_name, SCRIPT_NAME, pkg_dir, work_dir, function_name, function_name
    );

    run_shell(&script, fakeroot).await?;

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

fn get_dist_filepath(info: &Package) -> String {
    format!(
        "{}/{}-{}-{}.tar.zstd",
        DIST_DIR, info.name, info.version, info.release
    )
}

async fn is_exists_dist(info: &Package) -> anyhow::Result<bool> {
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

    fs::create_dir(SOURCES_DIR).await?;

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

        let command = format!("wget -q \"{}\" -P \"{}\"", source, SOURCES_DIR);
        run_shell(&command, false).await?;

        sleep(Duration::from_millis(DOWNLOAD_WAIT_TIME)).await;

        if !Path::new(&filepath).is_file() {
            anyhow::bail!("Downloaded file not found: {}", filename);
        }

        filepaths.push(filepath);
    }

    Ok(filepaths)
}

async fn check_sums(filepaths: &[String], info: &Package) -> anyhow::Result<()> {
    info!("Checking checksums...");

    for (i, filepath) in filepaths.iter().enumerate() {
        if i < info.sha256sums.len() && !info.sha256sums[i].is_empty() {
            hash::check_sum(filepath, &info.sha256sums[i], hash::Algorithm::Sha256).await?;
        }
        if i < info.sha512sums.len() && !info.sha512sums[i].is_empty() {
            hash::check_sum(filepath, &info.sha512sums[i], hash::Algorithm::Sha512).await?;
        }
        if i < info.b2sums.len() && !info.b2sums[i].is_empty() {
            hash::check_sum(filepath, &info.b2sums[i], hash::Algorithm::Blake2).await?;
        }
    }

    Ok(())
}

// TODO
// async fn verify_signatures(filepaths: &[String], validpgpkeys: &[String]) -> anyhow::Result<()> {
//     info!("Verifying signatures...");

//     let mut sign_files = Vec::new();
//     let mut source_files = Vec::new();

//     for filepath in filepaths {
//         if path::is_signature_file(filepath) {
//             sign_files.push(filepath);
//         } else {
//             source_files.push(filepath);
//         }
//     }

//     for sign_file in sign_files {
//         let base_name = Path::new(sign_file)
//             .file_stem()
//             .ok_or(anyhow::anyhow!("Failed to get file stem"))?
//             .to_string_lossy()
//             .to_string();

//         if let Some(source_file) = source_files
//             .iter()
//             .find(|f| {
//                 path::get_filename(f).is_some() && path::get_filename(f).unwrap() == base_name
//             })
//             .or(source_files.iter().find(|f| {
//                 path::get_filename(f).is_some() && path::get_filename(f).unwrap() == base_name
//             }))
//         {
//             println!("Verifying signature for file: {}", source_file);

//             let ks = KeyServer::new("hkps://keyserver.ubuntu.com")?;

//             let mut cert = Vec::new();
//             for fingerprint in validpgpkeys {
//                 let fingerprint = Fingerprint::from_hex(fingerprint)?;
//                 println!("Looking for key: {}", fingerprint);

//                 cert.push(
//                     ks.get(fingerprint)
//                         .await?
//                         .iter()
//                         .filter_map(|f| f.as_ref().ok().cloned())
//                         .collect::<Vec<_>>(),
//                 );
//             }

//             let mut source_content = Vec::new();
//             File::open(source_file)?.read_to_end(&mut source_content)?;

//             let mut sig_content = Vec::new();
//             File::open(sign_file)?.read_to_end(&mut sig_content)?;

//             println!("Verifying signature for file: {}", source_file);

//             let policy = &StandardPolicy::new();
//             DetachedVerifierBuilder::from_bytes(&sig_content)?
//                 .with_policy(
//                     policy,
//                     None,
//                     SimpleVerifier {
//                         cert: cert.iter().flat_map(|f| f.clone()).collect(),
//                     },
//                 )?
//                 .verify_bytes(source_content)?;

//             info!("Signature verified for file: {}", source_file);
//         } else {
//             anyhow::bail!(
//                 "No matching source file found for signature file: {}",
//                 sign_file
//             );
//         }
//     }

//     Ok(())
// }

async fn extract_sources(info: &Package) -> anyhow::Result<()> {
    info!("Extracting sources...");
    let (work_dir, _) = get_dirs(&info.name)?;
    let regex = Regex::new(r"http.*")?;

    for source in &info.sources {
        let filename = get_filename(source).ok_or(anyhow::anyhow!("Failed to get file name"))?;
        let filepath = format!("{}/{}", SOURCES_DIR, filename);

        if regex.is_match(source) {
            if path::is_tar_file(&filename) {
                let script = format!("tar -xf {} -C {} --strip-components=1", filepath, work_dir);
                run_shell(&script, false).await?;
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

async fn prepare(info: &Package) -> anyhow::Result<()> {
    info!("Preparing...");
    run_script_function(&info.name, "prepare", false).await?;
    Ok(())
}

async fn build(info: &Package) -> anyhow::Result<()> {
    info!("Building...");
    run_script_function(&info.name, "build", false).await?;
    Ok(())
}

async fn test(info: &Package) -> anyhow::Result<()> {
    info!("Testing...");
    run_script_function(&info.name, "test", false).await?;
    Ok(())
}

async fn package(info: &Package) -> anyhow::Result<()> {
    info!("Packaging...");
    run_script_function(&info.name, "package", true).await?;
    Ok(())
}

async fn create_archive(info: &Package) -> anyhow::Result<()> {
    info!("Creating archive...");

    let (_, pkg_dir) = get_dirs(&info.name)?;
    let output_name = format!("{}-{}-{}", info.name, info.version, info.release);

    fs::create_dir(DIST_DIR).await?;

    let script = format!(
        "tar -Izstd -cf '{}/{}.tar.zst' -C '{}' .",
        DIST_DIR, output_name, pkg_dir
    );

    run_shell(&script, true).await?;

    Ok(())
}

async fn sign_archive(info: &Package) -> anyhow::Result<()> {
    info!("Signing archive...");

    let output_name = format!("{}-{}-{}", info.name, info.version, info.release);

    let script = format!(
        "gpg --detach-sign --armor --output '{}/{}.tar.zst.asc' '{}/{}.tar.zst'",
        DIST_DIR, output_name, DIST_DIR, output_name
    );

    run_shell(&script, true).await?;

    Ok(())
}

async fn write_metadata(info: &Package) -> anyhow::Result<()> {
    info!("Writing package metadata...");

    let filepath = format!("{}/{}.meta", DIST_DIR, info.name);
    let content = format!("{}\n{}\n{}\n", info.name, info.version, info.release);

    fs::write_file(&filepath, &content).await?;

    Ok(())
}

async fn cleanup(info: &Package) -> anyhow::Result<()> {
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
        let info = Package::get(package_name, mirrors).await?;

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
        // verify_signatures(&filepaths, &info.validpgpkeys).await?;

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

    exit(0);
}
