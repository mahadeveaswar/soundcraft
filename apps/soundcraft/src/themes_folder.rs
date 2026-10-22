//! Finding the shared themes folder: the `themes/` folder of the source checkout the app was
//! built from, so themes saved there can be committed to git.
//!
//! The app only uses it when it runs from a cargo build folder (`<root>/target/{release,debug}`,
//! or `target/<triple>/...`) of a checkout that has a `Cargo.toml` and a `themes/` folder. An
//! installed app or a macOS bundle never matches, so it never writes into a random folder.

use std::path::{Path, PathBuf};

/// The shared themes folder for an executable in `exe_dir`, or `None`. `exists` answers whether a
/// path exists (a parameter so the rule can be tested without a checkout).
pub fn detect(exe_dir: &Path, exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    for target in exe_dir.ancestors() {
        if target.file_name().is_none_or(|n| n != "target") {
            continue;
        }
        let root = target.parent()?;
        // `release` or `debug` somewhere between `target` and the executable (`debug/deps` for tests).
        let built = exe_dir.strip_prefix(target).ok()?.components().any(|c| c.as_os_str() == "release" || c.as_os_str() == "debug");
        let themes = root.join("themes");
        if built && exists(&root.join("Cargo.toml")) && exists(&themes) {
            return Some(themes);
        }
    }
    None
}

/// [`detect`] for the running executable.
pub fn detect_for_running_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    detect(exe.parent()?, Path::exists)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(files: &'static [&'static str]) -> impl Fn(&Path) -> bool {
        move |p: &Path| files.iter().any(|f| p == Path::new(f))
    }

    #[test]
    fn a_cargo_build_folder_of_a_checkout_finds_its_themes() {
        let exists = fake(&["/src/sc/Cargo.toml", "/src/sc/themes"]);
        let want = Some(PathBuf::from("/src/sc/themes"));
        assert_eq!(detect(Path::new("/src/sc/target/release"), &exists), want);
        assert_eq!(detect(Path::new("/src/sc/target/debug"), &exists), want);
        assert_eq!(detect(Path::new("/src/sc/target/debug/deps"), &exists), want);
        assert_eq!(detect(Path::new("/src/sc/target/x86_64-unknown-linux-gnu/release"), &exists), want);
    }

    #[test]
    fn installed_apps_bundles_and_incomplete_checkouts_find_nothing() {
        let exists = fake(&["/src/sc/Cargo.toml", "/src/sc/themes", "/other/Cargo.toml"]);
        assert_eq!(detect(Path::new("/Applications/SoundCraft.app/Contents/MacOS"), &exists), None);
        assert_eq!(detect(Path::new("/usr/local/bin"), &exists), None);
        assert_eq!(detect(Path::new("/src/sc"), &exists), None, "not inside target");
        assert_eq!(detect(Path::new("/src/sc/target/package"), &exists), None, "not a build profile folder");
        assert_eq!(detect(Path::new("/other/target/release"), &exists), None, "no themes folder");
        assert_eq!(detect(Path::new("/src/sc/target/release"), fake(&["/src/sc/themes"])), None, "no Cargo.toml");
    }

    #[test]
    fn the_real_folders_of_a_temporary_checkout_are_found() {
        let root = std::env::temp_dir().join(format!("soundcraft-detect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let exe_dir = root.join("target").join("release");
        std::fs::create_dir_all(&exe_dir).expect("temp folders");
        std::fs::create_dir_all(root.join("themes")).expect("themes folder");
        assert_eq!(detect(&exe_dir, Path::exists), None, "no Cargo.toml yet");
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("manifest");
        assert_eq!(detect(&exe_dir, Path::exists), Some(root.join("themes")));
        let _ = std::fs::remove_dir_all(&root);
    }
}
