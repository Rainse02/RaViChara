use crate::config::AppConfig;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const PORTABLE_MARKER: &str = "RaViChara.portable";
const LEGACY_PORTABLE_MARKER: &str = "EverChara.portable";

pub fn prepare_runtime_root(explicit: Option<&Path>) -> io::Result<PathBuf> {
    let root = choose_runtime_root(explicit)?;
    seed_runtime_root(&root)?;
    env::set_current_dir(&root)?;
    Ok(root)
}

fn choose_runtime_root(explicit: Option<&Path>) -> io::Result<PathBuf> {
    if let Some(path) = explicit {
        return absolute_path(path);
    }
    if let Some(path) = env::var_os("RAVICHARA_HOME").filter(|value| !value.is_empty()) {
        return absolute_path(Path::new(&path));
    }
    // Keep the old variable for existing installations; new documentation and
    // packages use RAVICHARA_HOME.
    if let Some(path) = env::var_os("EVERCHARA_HOME").filter(|value| !value.is_empty()) {
        return absolute_path(Path::new(&path));
    }

    let executable = env::current_exe()?;
    if let Some(directory) = executable.parent() {
        if directory.join(PORTABLE_MARKER).is_file()
            || directory.join(LEGACY_PORTABLE_MARKER).is_file()
        {
            return Ok(directory.to_path_buf());
        }
    }

    let current = env::current_dir()?;
    if current.join("Cargo.toml").is_file()
        && current.join("config/settings.yaml").is_file()
        && current.join("characters").is_dir()
    {
        return Ok(current);
    }

    let local_data = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| executable.parent().map(PathBuf::from))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "unable to resolve a writable RaViChara data directory",
            )
        })?;
    let root = local_data.join("RaViChara");
    let legacy = local_data.join("EverChara");
    // Existing non-portable users keep their original data in place instead
    // of receiving a second copy of chat history and memories. A new install
    // starts under the RaViChara directory.
    if !root.exists() && legacy.exists() {
        Ok(legacy)
    } else {
        Ok(root)
    }
}

fn absolute_path(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

pub fn seed_runtime_root(root: &Path) -> io::Result<()> {
    let settings_directory = root.join("config");
    let characters_directory = root.join("characters");
    let data_directory = root.join("data");
    for directory in [
        settings_directory.as_path(),
        characters_directory.as_path(),
        data_directory.join("memories").as_path(),
        root.join("logs").as_path(),
        root.join("webview").as_path(),
    ] {
        fs::create_dir_all(directory)?;
    }

    let settings = serde_yaml::to_string(&AppConfig::default())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write_if_missing(
        &settings_directory.join("settings.yaml"),
        settings.as_bytes(),
    )?;
    write_if_missing(
        &characters_directory.join("lily.card.yaml"),
        include_bytes!("../characters/lily.card.yaml"),
    )?;
    write_if_missing(
        &characters_directory.join("lily.avatar.svg"),
        include_bytes!("../characters/lily.avatar.svg"),
    )?;
    Ok(())
}

fn write_if_missing(path: &Path, content: &[u8]) -> io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::seed_runtime_root;
    use crate::config::AppConfig;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_directory() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "ravichara-runtime-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn desktop_runtime_is_seeded_without_secrets_or_overwrites() {
        let root = test_directory();
        seed_runtime_root(&root).unwrap();
        let settings = fs::read_to_string(root.join("config/settings.yaml")).unwrap();
        let config: AppConfig = serde_yaml::from_str(&settings).unwrap();
        assert!(config.llm.api_key.is_empty());
        assert!(config.voice.tts.api_key.is_empty());
        assert!(config.blender.token.is_empty());
        assert!(root.join("characters/lily.card.yaml").is_file());
        assert!(root.join("characters/lily.avatar.svg").is_file());
        assert!(root.join("data/memories").is_dir());

        fs::write(root.join("characters/lily.card.yaml"), "custom").unwrap();
        seed_runtime_root(&root).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("characters/lily.card.yaml")).unwrap(),
            "custom"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
