use sha2::Digest;
use tokio::{fs::File, io::AsyncReadExt};

fn hash_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub enum Algorithm {
    Sha256,
    Sha512,
    Blake2,
}

async fn get_hash_from_file<H: Digest + Default>(mut file: File) -> anyhow::Result<String> {
    let mut hasher = H::default();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hash_to_hex(&hasher.finalize()))
}

pub async fn check_sum(filepath: &str, sum: &str, algo: Algorithm) -> anyhow::Result<()> {
    let file = super::fs::open_file(filepath).await?;
    let hash = match algo {
        Algorithm::Sha256 => get_hash_from_file::<sha2::Sha256>(file).await?,
        Algorithm::Sha512 => get_hash_from_file::<sha2::Sha512>(file).await?,
        Algorithm::Blake2 => get_hash_from_file::<blake2::Blake2b512>(file).await?,
    };

    if sum == hash || sum == "SKIP" {
        Ok(())
    } else {
        anyhow::bail!(
            "Checksum mismatch for {}: expected {}, got {}",
            filepath,
            sum,
            hash
        );
    }
}
