//! Folders that 2DriveScan never moves to the Trash, so one wrong drag cannot
//! break the system or every app's settings.
//!
//! Two kinds of lock:
//! - **Whole tree**: the folder and everything in it (`/System`, `/usr`,
//!   `C:\Windows`, ...). Nothing in there should be removed by hand.
//! - **Folder only**: the folder itself stays, its contents can go (`/Library`,
//!   the home folder, `~/Library/Caches`, Downloads, ...). Cleanup suggestions
//!   point inside several of these.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct Rules {
    /// Locked with everything inside.
    trees: Vec<PathBuf>,
    /// Inside a locked tree, but not locked (`/usr/local`).
    exceptions: Vec<PathBuf>,
    /// Only the folder itself is locked.
    folders: Vec<PathBuf>,
}

/// Paths are compared case-insensitively where the file system usually is.
fn key(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if cfg!(any(target_os = "macos", windows)) {
        PathBuf::from(s.to_lowercase())
    } else {
        PathBuf::from(s.into_owned())
    }
}

impl Rules {
    fn new(trees: Vec<PathBuf>, exceptions: Vec<PathBuf>, folders: Vec<PathBuf>) -> Self {
        let keys = |v: Vec<PathBuf>| v.iter().map(|p| key(p)).collect();
        Rules {
            trees: keys(trees),
            exceptions: keys(exceptions),
            folders: keys(folders),
        }
    }

    /// The rules for this computer.
    pub fn get() -> &'static Rules {
        static RULES: OnceLock<Rules> = OnceLock::new();
        RULES.get_or_init(|| Rules::for_home(home().as_deref()))
    }

    pub fn for_home(home: Option<&Path>) -> Rules {
        let (trees, exceptions, mut folders, home_folders) = system_rules();
        if let Some(home) = home {
            folders.push(home.to_path_buf());
            folders.extend(home_folders.iter().map(|rel| home.join(rel)));
        }
        Rules::new(trees, exceptions, folders)
    }

    pub fn is_locked(&self, path: &Path) -> bool {
        let p = key(&normalize(path));
        if self.folders.contains(&p) {
            return true;
        }
        if inside_photos_library(&p) {
            return true;
        }
        if self.exceptions.iter().any(|e| p.starts_with(e)) {
            return false;
        }
        self.trees.iter().any(|t| p.starts_with(t))
    }
}

/// Shorthand for the rules of this computer.
pub fn is_locked(path: &Path) -> bool {
    Rules::get().is_locked(path)
}

fn home() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

/// On macOS the data volume can also be scanned through its own mount point;
/// map those paths to where they normally appear.
fn normalize(path: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Ok(rest) = path.strip_prefix("/System/Volumes/Data") {
            return Path::new("/").join(rest);
        }
    }
    path.to_path_buf()
}

/// Files inside a Photos library are managed by Photos; removing some of
/// them damages the library. (The library as a whole can still go.)
fn inside_photos_library(p: &Path) -> bool {
    let mut parents = p.ancestors().skip(1);
    parents.any(|a| {
        a.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("photoslibrary"))
    })
}

type SystemRules = (
    Vec<PathBuf>,
    Vec<PathBuf>,
    Vec<PathBuf>,
    &'static [&'static str],
);

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

#[cfg(target_os = "macos")]
fn system_rules() -> SystemRules {
    (
        paths(&[
            "/System",
            "/bin",
            "/sbin",
            "/usr",
            "/private",
            "/etc",
            "/var",
            "/tmp",
            "/cores",
            "/dev",
            "/Library/Apple",
        ]),
        paths(&["/usr/local"]),
        paths(&[
            "/",
            "/Applications",
            "/Library",
            "/Library/Application Support",
            "/Users",
            "/Users/Shared",
            "/opt",
            "/usr/local",
        ]),
        &[
            "Applications",
            "Desktop",
            "Documents",
            "Downloads",
            "Movies",
            "Music",
            "Pictures",
            "Public",
            "Library",
            "Library/Application Support",
            "Library/Caches",
            "Library/Containers",
            "Library/Group Containers",
            "Library/Logs",
            "Library/Mobile Documents",
            "Library/Preferences",
            ".Trash",
        ],
    )
}

#[cfg(windows)]
fn system_rules() -> SystemRules {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let env = |name: &str, fallback: &str| {
        std::env::var(name)
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(format!("{drive}\\{fallback}")))
    };
    let at = |rel: &str| PathBuf::from(format!("{drive}\\{rel}"));
    (
        vec![
            env("SystemRoot", "Windows"),
            at("System Volume Information"),
            at("$Recycle.Bin"),
            at("Recovery"),
            at("pagefile.sys"),
            at("hiberfil.sys"),
            at("swapfile.sys"),
        ],
        Vec::new(),
        vec![
            at(""),
            env("ProgramFiles", "Program Files"),
            env("ProgramFiles(x86)", "Program Files (x86)"),
            env("ProgramData", "ProgramData"),
            at("Users"),
        ],
        &[
            "AppData",
            "AppData/Local",
            "AppData/Local/Temp",
            "AppData/LocalLow",
            "AppData/Roaming",
            "Desktop",
            "Documents",
            "Downloads",
            "Music",
            "Pictures",
            "Videos",
        ],
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn system_rules() -> SystemRules {
    (
        paths(&[
            "/bin",
            "/sbin",
            "/lib",
            "/lib32",
            "/lib64",
            "/libx32",
            "/usr",
            "/etc",
            "/boot",
            "/proc",
            "/sys",
            "/dev",
            "/run",
            "/snap",
            "/var/lib/dpkg",
            "/var/lib/rpm",
            "/var/lib/pacman",
            "/var/lib/apt",
        ]),
        paths(&["/usr/local"]),
        paths(&[
            "/", "/home", "/root", "/opt", "/var", "/var/lib", "/var/log", "/srv", "/mnt", "/media",
        ]),
        &[
            ".cache",
            ".config",
            ".local",
            ".local/share",
            "Desktop",
            "Documents",
            "Downloads",
            "Music",
            "Pictures",
            "Videos",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn locks_system_trees_and_key_folders_only() {
        let rules = Rules::for_home(Some(Path::new("/home/ann")));
        let locked = |p: &str| rules.is_locked(Path::new(p));
        // Whole trees.
        assert!(locked("/usr"));
        assert!(locked("/usr/lib/libc.so"));
        assert!(locked("/etc/hosts"));
        // Exception inside a locked tree.
        assert!(!locked("/usr/local/bin/tool"));
        // The folder itself, not its contents.
        assert!(locked("/"));
        assert!(locked("/home/ann"));
        assert!(!locked("/home/ann/project"));
        assert!(locked("/home/ann/Downloads"));
        assert!(!locked("/home/ann/Downloads/big.iso"));
        // Photos libraries: the library can go, its insides cannot.
        assert!(!locked("/home/ann/Pictures/Photos Library.photoslibrary"));
        assert!(locked(
            "/home/ann/Pictures/Photos Library.photoslibrary/originals/a.heic"
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_rules() {
        let rules = Rules::for_home(Some(Path::new("/Users/ann")));
        let locked = |p: &str| rules.is_locked(Path::new(p));
        assert!(locked("/System/Library/CoreServices"));
        assert!(locked("/private/var/db"));
        assert!(locked("/Library"));
        assert!(!locked("/Library/Caches"));
        assert!(locked("/Users/ann/Library"));
        assert!(locked("/users/ann/library")); // case-insensitive
        assert!(locked("/Users/ann/Library/Caches"));
        assert!(!locked("/Users/ann/Library/Caches/com.example.app"));
        assert!(!locked("/Applications/Some.app"));
        // The data volume seen through its own mount point.
        assert!(locked("/System/Volumes/Data/Users/ann"));
        assert!(!locked("/System/Volumes/Data/Users/ann/Movies/clip.mov"));
    }
}
