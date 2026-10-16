//! Shell choice for native process tests. The runner checks full custody.
pub(super) fn sh() -> &'static str {
    #[cfg(target_os = "linux")]
    const CANDIDATES: &[&str] = &["/usr/bin/dash", "/usr/bin/bash"];
    #[cfg(target_os = "macos")]
    const CANDIDATES: &[&str] = &["/bin/sh"];
    shell_from(CANDIDATES).expect("no regular POSIX shell for native runner tests")
}

fn shell_from<'a>(candidates: &[&'a str]) -> Option<&'a str> {
    use std::os::unix::fs::MetadataExt;
    candidates.iter().copied().find(|path| {
        std::fs::symlink_metadata(path)
            .is_ok_and(|meta| meta.file_type().is_file() && meta.nlink() == 1)
    })
}

#[test]
fn missing_or_linked_shells_do_not_hide_a_regular_fallback() {
    use crate::broker::private_temp_harness::PrivateTemp;
    use std::fs;
    use std::os::unix::fs::symlink;

    let temp = PrivateTemp::new("hush-shell");
    let missing = temp.path().join("missing");
    let valid = temp.path().join("valid");
    let linked = temp.path().join("linked");
    let hard = temp.path().join("hard");
    let alias = temp.path().join("alias");
    fs::write(&valid, b"synthetic shell candidate").unwrap();
    fs::write(&hard, b"two links").unwrap();
    fs::hard_link(&hard, &alias).unwrap();
    symlink(&valid, &linked).unwrap();
    let names = [&missing, &linked, &hard, &valid].map(|path| path.to_str().unwrap());
    assert_eq!(
        shell_from(&names),
        Some(valid.to_str().unwrap()),
        "a missing preferred shell must use the regular fallback"
    );
    for path in [&missing, &linked, &hard] {
        assert_eq!(shell_from(&[path.to_str().unwrap()]), None);
    }
}
