//! Classifies files into a handful of human friendly categories based on
//! their extension. The categories drive the "File types" breakdown and the
//! "color by type" mode of the sunburst.

use serde::Serialize;

/// Number of categories, used to size per-directory breakdown arrays.
pub const CATEGORY_COUNT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum Category {
    Video = 0,
    Audio = 1,
    Image = 2,
    Document = 3,
    Archive = 4,
    Code = 5,
    App = 6,
    Other = 7,
}

impl Category {
    pub const ALL: [Category; CATEGORY_COUNT] = [
        Category::Video,
        Category::Audio,
        Category::Image,
        Category::Document,
        Category::Archive,
        Category::Code,
        Category::App,
        Category::Other,
    ];

    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(i: usize) -> Category {
        Category::ALL.get(i).copied().unwrap_or(Category::Other)
    }
}

/// Categorize a file by its name. Matching is case-insensitive and does not
/// allocate.
pub fn categorize(name: &str) -> Category {
    let ext = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && ext.len() <= 12 => ext,
        _ => return Category::Other,
    };
    let mut buf = [0u8; 12];
    for (dst, src) in buf.iter_mut().zip(ext.bytes()) {
        *dst = src.to_ascii_lowercase();
    }
    let lower = match std::str::from_utf8(&buf[..ext.len()]) {
        Ok(s) => s,
        Err(_) => return Category::Other,
    };
    match lower {
        // Video
        "mp4" | "m4v" | "mov" | "avi" | "mkv" | "wmv" | "flv" | "webm" | "mpg" | "mpeg" | "3gp"
        | "mts" | "m2ts" | "vob" | "ogv" | "braw" | "r3d" | "mxf" | "dv" | "insv" | "lrv" => {
            Category::Video
        }
        // Audio
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "oga" | "opus" | "wma" | "aiff"
        | "aif" | "alac" | "mid" | "midi" | "amr" | "ape" | "caf" | "m4b" | "m4p" | "dsf"
        | "dff" => Category::Audio,
        // Images & design
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tif" | "tiff" | "webp" | "heic" | "heif"
        | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "orf" | "rw2" | "raf" | "psd" | "psb"
        | "ai" | "svg" | "ico" | "icns" | "xcf" | "avif" | "jxl" | "exr" | "hdr" | "sketch"
        | "fig" | "afphoto" | "afdesign" | "kra" | "blend" | "eps" => Category::Image,
        // Documents
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "xlsm" | "ppt" | "pptx" | "odt" | "ods"
        | "odp" | "rtf" | "txt" | "md" | "markdown" | "pages" | "numbers" | "key" | "epub"
        | "mobi" | "azw3" | "csv" | "tsv" | "tex" | "vsdx" | "one" | "msg" | "eml" | "mbox"
        | "dotx" => Category::Document,
        // Archives & disk images
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "lz4" | "lzma"
        | "cab" | "dmg" | "iso" | "img" | "vmdk" | "vdi" | "vhd" | "vhdx" | "qcow2"
        | "sparseimage" | "toast" | "wim" | "esd" | "ipsw" | "xip" | "tbz" | "txz" | "zipx"
        | "ova" => Category::Archive,
        // Source code & developer artefacts
        "rs" | "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" | "py" | "pyc" | "rb" | "go"
        | "java" | "class" | "jar" | "kt" | "swift" | "c" | "h" | "cc" | "cpp" | "hpp" | "cs"
        | "m" | "mm" | "php" | "html" | "htm" | "css" | "scss" | "sass" | "less" | "json"
        | "yaml" | "yml" | "toml" | "xml" | "sql" | "sh" | "bash" | "zsh" | "ps1" | "bat"
        | "lua" | "r" | "dart" | "vue" | "svelte" | "ipynb" | "o" | "obj" | "a" | "rlib"
        | "rmeta" | "pdb" | "d" | "lock" | "map" | "wasm" | "gradle" | "proto" | "el" | "pack"
        | "idx" | "node" | "whl" | "gem" | "crate" | "sqlite" | "db" => Category::Code,
        // Applications & binaries
        "exe" | "msi" | "dll" | "so" | "dylib" | "pkg" | "deb" | "rpm" | "appimage" | "apk"
        | "aab" | "ipa" | "bin" | "sys" | "drv" | "ko" | "efi" | "msix" | "appx" | "snap"
        | "flatpak" | "mui" => Category::App,
        _ => Category::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorizes_common_extensions() {
        assert_eq!(categorize("holiday.MP4"), Category::Video);
        assert_eq!(categorize("song.flac"), Category::Audio);
        assert_eq!(categorize("IMG_0001.HEIC"), Category::Image);
        assert_eq!(categorize("report.pdf"), Category::Document);
        assert_eq!(categorize("backup.tar.gz"), Category::Archive);
        assert_eq!(categorize("main.rs"), Category::Code);
        assert_eq!(categorize("setup.exe"), Category::App);
        assert_eq!(categorize("README"), Category::Other);
        assert_eq!(categorize(".bashrc"), Category::Other);
        assert_eq!(
            categorize("weird.ext-that-is-way-too-long"),
            Category::Other
        );
    }
}
