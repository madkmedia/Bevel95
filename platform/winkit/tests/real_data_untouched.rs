//! The tests never write the real data folder (the second safety review of 2026-11-18, C3): a
//! test build puts its log (and every state file) into a scratch folder instead of
//! %LOCALAPPDATA%\Bevel95. Before, every log line of a test went into the bar's own log, and on a
//! contributor's PC `cargo test` made the folder.

use std::path::PathBuf;

fn real_folder() -> PathBuf {
    bevel_winkit::shell::known_folder(&windows_sys::Win32::UI::Shell::FOLDERID_LocalAppData)
        .expect("Windows knows the local application data folder")
        .join("Bevel95")
}

#[test]
fn a_test_run_never_writes_the_real_data_folder() {
    let real = real_folder();
    let was_there = real.exists();
    assert_ne!(bevel_winkit::log::installed_dir(), real, "a test build uses a scratch folder");
    let marker = format!(
        "a test line that must not reach the real log ({} {:?})",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap()
    );
    bevel_winkit::log::log(&marker);
    let scratch_log = bevel_winkit::log::data_dir().join("bevel95.log");
    assert!(std::fs::read_to_string(&scratch_log).unwrap().contains(&marker), "the line went to the scratch log");
    assert_eq!(real.exists(), was_there, "the real folder was not made");
    for name in ["bevel95.log", "bevel95.old.log"] {
        if let Ok(text) = std::fs::read(real.join(name)) {
            assert!(!String::from_utf8_lossy(&text).contains(&marker), "the real {name} got the test's line");
        }
    }
}
