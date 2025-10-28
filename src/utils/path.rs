use std::path::Path;

use regex::Regex;

pub fn is_tar_file(filename: &str) -> bool {
    let regex = Regex::new(r".*\.tar\.(bz2|gz|lz4|xz|zst)$|.*\.tgz$").unwrap();
    regex.is_match(filename)
}

pub fn is_signature_file(filename: &str) -> bool {
    let regex = Regex::new(r".*\.(sig|sig|asc)$").unwrap();
    regex.is_match(filename)
}

pub fn get_filename(filepath: &str) -> Option<String> {
    let filename = Path::new(filepath)
        .file_name()?
        .to_string_lossy()
        .to_string();

    Some(filename)
}
