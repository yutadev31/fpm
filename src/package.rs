pub struct PkgInfo {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) release: String,
    pub(crate) sources: Vec<String>,
    pub(crate) sha256sums: Vec<String>,
    pub(crate) sha512sums: Vec<String>,
    pub(crate) b2sums: Vec<String>,
    pub(crate) validpgpkeys: Vec<String>,
    pub(crate) dependencies: Vec<String>,
}
