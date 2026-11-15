//! Gives the program file its icon and its name and version details: what Explorer, shortcuts,
//! the task bar and Windows' list of installed apps show for it. The icon is drawn by
//! src/icon_art.rs; the toolchain's resource compiler turns it into an object file that is
//! linked into the program. If that tool is missing the program is built without them, with a
//! warning, rather than not at all.

use std::path::{Path, PathBuf};
use std::process::Command;

include!("src/icon_art.rs");

/// The sizes Windows picks from: small lists, the task bar, large icons, the 256-pixel tile.
const ICON_SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

/// One picture of an .ico file: a 32-bit bitmap, bottom row first, with its 1-bit mask behind it.
fn icon_picture(size: u32) -> Vec<u8> {
    let rgba = icon_rgba(size);
    let mask_row = ((size + 31) / 32 * 4) as usize;
    let mut d = Vec::new();
    d.extend(40u32.to_le_bytes());
    d.extend((size as i32).to_le_bytes());
    d.extend((size as i32 * 2).to_le_bytes());
    d.extend(1u16.to_le_bytes());
    d.extend(32u16.to_le_bytes());
    d.extend(0u32.to_le_bytes());
    d.extend((size * size * 4 + mask_row as u32 * size).to_le_bytes());
    d.extend([0u8; 16]);
    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            d.extend([rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    for y in (0..size).rev() {
        let mut row = vec![0u8; mask_row];
        for x in 0..size {
            if rgba[((y * size + x) * 4 + 3) as usize] == 0 {
                row[(x / 8) as usize] |= 0x80 >> (x % 8);
            }
        }
        d.extend(row);
    }
    d
}

fn icon_file() -> Vec<u8> {
    let pictures: Vec<Vec<u8>> = ICON_SIZES.iter().map(|&s| icon_picture(s)).collect();
    let mut ico = vec![0u8, 0, 1, 0];
    ico.extend((ICON_SIZES.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * ICON_SIZES.len();
    for (size, picture) in ICON_SIZES.iter().zip(&pictures) {
        // A width or height of 256 is written as 0.
        ico.extend([(size % 256) as u8, (size % 256) as u8, 0, 0]);
        ico.extend(1u16.to_le_bytes());
        ico.extend(32u16.to_le_bytes());
        ico.extend((picture.len() as u32).to_le_bytes());
        ico.extend((offset as u32).to_le_bytes());
        offset += picture.len();
    }
    for picture in pictures {
        ico.extend(picture);
    }
    ico
}

fn resource_script(version: &str) -> String {
    let mut parts: Vec<u32> = version.split('.').filter_map(|p| p.parse().ok()).collect();
    parts.resize(4, 0);
    let numbers = format!("{},{},{},{}", parts[0], parts[1], parts[2], parts[3]);
    format!(
        r#"1 ICON "app.ico"
1 VERSIONINFO
FILEVERSION {numbers}
PRODUCTVERSION {numbers}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "FileDescription", "Event Studio"
      VALUE "ProductName", "Event Studio"
      VALUE "FileVersion", "{version}"
      VALUE "ProductVersion", "{version}"
      VALUE "InternalName", "event-studio"
      VALUE "OriginalFilename", "EventStudio.exe"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    )
}

/// The resource compiler beside the configured linker (.cargo/config.toml), else whatever `windres` the PATH has.
fn resource_compiler() -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Ok(linker) = std::env::var("RUSTC_LINKER") {
        let linker = PathBuf::from(linker);
        let linker = if linker.is_absolute() { linker } else { Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()).join(linker) };
        if let Some(bin) = linker.parent() {
            found.extend(["x86_64-w64-mingw32-windres.exe", "windres.exe"].map(|name| bin.join(name)).into_iter().filter(|p| p.is_file()));
        }
    }
    found.push(PathBuf::from("windres"));
    found
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon_art.rs");
    let target = |name: &str| std::env::var(format!("CARGO_CFG_TARGET_{name}")).unwrap_or_default();
    if target("OS") != "windows" || target("ARCH") != "x86_64" {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let written = std::fs::write(out.join("app.ico"), icon_file()).and_then(|_| std::fs::write(out.join("app.rc"), resource_script(&version)));
    if let Err(e) = written {
        println!("cargo:warning=no icon in the program file: {e}");
        return;
    }
    let object = out.join("app_resources.o");
    let _ = std::fs::remove_file(&object);
    for tool in resource_compiler() {
        let run = Command::new(&tool).current_dir(&out).args(["--input", "app.rc", "--output", "app_resources.o", "--output-format=coff"]).output();
        if run.is_ok_and(|r| r.status.success()) && object.is_file() {
            println!("cargo:rustc-link-arg-bins={}", object.display());
            return;
        }
    }
    println!("cargo:warning=no icon in the program file: the resource compiler (windres) did not run");
}
