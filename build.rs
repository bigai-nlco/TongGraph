use std::env;
use std::path::{Path, PathBuf};

fn main() {
    pyo3_build_config::add_extension_module_link_args();
    add_sqlite_link_fallback();
}

fn add_sqlite_link_fallback() {
    println!("cargo:rerun-if-env-changed=LIBRARY_PATH");
    println!("cargo:rerun-if-env-changed=TONGGRAPH_SQLITE3_LIB_DIR");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }

    if let Some(dir) = explicit_sqlite_lib_dir() {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }

    if find_sqlite_library("libsqlite3.so").is_some() {
        return;
    }

    let Some(versioned_sqlite) = find_sqlite_library("libsqlite3.so.0") else {
        return;
    };

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let linker_name = out_dir.join("libsqlite3.so");

    if std::fs::symlink_metadata(&linker_name).is_ok() {
        std::fs::remove_file(&linker_name)
            .expect("failed to remove stale SQLite linker-name symlink");
    }

    create_linker_name(&versioned_sqlite, &linker_name);
    println!("cargo:rustc-link-search=native={}", out_dir.display());
}

#[cfg(unix)]
fn create_linker_name(versioned_sqlite: &Path, linker_name: &Path) {
    std::os::unix::fs::symlink(versioned_sqlite, linker_name)
        .expect("failed to create SQLite linker-name symlink");
}

#[cfg(not(unix))]
fn create_linker_name(versioned_sqlite: &Path, linker_name: &Path) {
    std::fs::copy(versioned_sqlite, linker_name)
        .expect("failed to copy SQLite linker-name library");
}

fn find_sqlite_library(file_name: &str) -> Option<PathBuf> {
    sqlite_search_dirs()
        .into_iter()
        .map(|dir| dir.join(file_name))
        .find(|path| path.exists())
}

fn explicit_sqlite_lib_dir() -> Option<PathBuf> {
    env::var_os("TONGGRAPH_SQLITE3_LIB_DIR").map(PathBuf::from)
}

fn sqlite_search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(dir) = explicit_sqlite_lib_dir() {
        dirs.push(dir);
    }

    if let Some(paths) = env::var_os("LIBRARY_PATH") {
        dirs.extend(env::split_paths(&paths));
    }

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let multiarch = match target_arch.as_str() {
        "aarch64" => Some("aarch64-linux-gnu"),
        "arm" => Some("arm-linux-gnueabihf"),
        "powerpc64" => Some("powerpc64-linux-gnu"),
        "riscv64" => Some("riscv64-linux-gnu"),
        "s390x" => Some("s390x-linux-gnu"),
        "x86" => Some("i386-linux-gnu"),
        "x86_64" => Some("x86_64-linux-gnu"),
        _ => None,
    };

    if let Some(multiarch) = multiarch {
        dirs.push(Path::new("/lib").join(multiarch));
        dirs.push(Path::new("/usr/lib").join(multiarch));
    }

    dirs.extend(
        ["/lib64", "/usr/lib64", "/lib", "/usr/lib"]
            .into_iter()
            .map(PathBuf::from),
    );

    dirs
}
