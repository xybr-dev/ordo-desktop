use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The installation-local preferences file. Parsing remains application-owned.
#[derive(Clone, Debug)]
pub struct PreferencesFile(PathBuf);

impl PreferencesFile {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn read(&self) -> io::Result<Option<String>> {
        match fs::read_to_string(&self.0) {
            Ok(contents) => Ok(Some(contents)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub fn write(&self, contents: &str) -> io::Result<()> {
        if let Some(parent) = self.0.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.0, contents)
    }
}

pub fn installation_preferences_path() -> io::Result<PathBuf> {
    config_root().map(|root| root.join("ordo").join("preferences"))
}

#[cfg(target_os = "windows")]
fn config_root() -> io::Result<PathBuf> {
    std::env::var_os("APPDATA")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA is not set"))
}

#[cfg(not(target_os = "windows"))]
fn config_root() -> io::Result<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_CONFIG_HOME").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    std::env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(|path| Path::new(&path).join(".config"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME is not set"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn preferences_file_round_trips_in_a_temporary_location() {
        let path = std::env::temp_dir().join(format!(
            "ordo-preferences-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let file = PreferencesFile::new(&path);

        assert_eq!(file.read().unwrap(), None);
        file.write("appearance=dark\nscale=175\n").unwrap();
        assert_eq!(
            file.read().unwrap().as_deref(),
            Some("appearance=dark\nscale=175\n")
        );

        fs::remove_file(path).unwrap();
    }
}
