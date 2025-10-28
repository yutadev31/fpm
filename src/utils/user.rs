pub fn is_user_root() -> bool {
    let uid = unsafe { nix::libc::getuid() };
    uid == 0
}
