use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;


fn main() {
    println!("cargo:rerun-if-changed=resources/windows.rc");
    println!("cargo:rerun-if-changed=assets/ravichara.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let Some(resource_compiler) = find_resource_compiler() else {
        println!(
            "cargo:warning=Windows SDK rc.exe was not found; the runtime window icon remains available, but the executable file icon cannot be embedded"
        );
        return;
    };
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"),
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set"))
        .join("ravichara.res");
    let status = Command::new(&resource_compiler)
        .current_dir(&manifest_dir)
        .arg("/nologo")
        .arg("/fo")
        .arg(&output)
        .arg(manifest_dir.join("resources").join("windows.rc"))
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "failed to launch Windows resource compiler {}: {error}",
                resource_compiler.display()
            )
        });
    assert!(status.success(), "Windows resource compilation failed");
    println!("cargo:rustc-link-arg={}", output.display());
}


fn find_resource_compiler() -> Option<PathBuf> {
    if let Some(path) = env::var_os("RC").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }
    let architecture = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x64",
        Ok("x86") => "x86",
        Ok("aarch64") => "arm64",
        _ => "x64",
    };
    let program_files = env::var_os("ProgramFiles(x86)")
        .or_else(|| env::var_os("ProgramFiles"))?;
    let bin_root = Path::new(&program_files)
        .join("Windows Kits")
        .join("10")
        .join("bin");
    let mut versions = std::fs::read_dir(bin_root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    versions.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    versions
        .into_iter()
        .map(|version| version.join(architecture).join("rc.exe"))
        .find(|candidate| candidate.is_file())
}
