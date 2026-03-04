use std::fmt::{self, Display};
use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug)]
pub enum RegistryError {
    ReadError(std::io::Error),
    FormatError {
        message: String,
        details: Vec<String>,
        source: Option<Box<dyn std::error::Error>>,
    },
}

impl Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::ReadError(e) => write!(f, "failed to read registry content: {}", e),
            RegistryError::FormatError {
                message,
                details,
                source,
            } => match source {
                Some(e) => write!(f, "{}: {} - {}", message, details.join(", "), e),
                None => write!(f, "{} - {}", message, details.join(", ")),
            },
        }
    }
}

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RegistryError::ReadError(e) => Some(e),
            RegistryError::FormatError { .. } => None,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct PackageSpec {
    pub name: String,
    pub description: String,
}

#[derive(Deserialize, Debug)]
pub struct VersionSpec {
    pub version: String,
    pub source: SourceSpec,
    pub signature: SignatureSpec,
    pub build: BuildSpec,
}

#[derive(Deserialize, Debug)]
pub struct SourceSpec {
    pub url: String,
    pub sha256: String,
}

#[derive(Deserialize, Debug)]
pub struct SignatureSpec {
    pub r#type: String,
    pub url: String,
    pub expected_keys: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct BuildSpec {
    pub system: String,
    pub configure_args: Vec<String>,
}

pub trait Registry {
    fn search_packages(&self, term: String) -> Result<Vec<PackageSpec>, RegistryError>;
    fn list_versions(&self, package: &str) -> Result<Vec<String>, RegistryError>;
    fn get_version_spec(
        &self,
        package: &str,
        version: &str,
    ) -> Result<Option<VersionSpec>, RegistryError>;
}

pub struct FileRegistry {
    path: std::path::PathBuf,
}

impl FileRegistry {
    pub fn new(path: &Path) -> Self {
        FileRegistry {
            path: path.to_path_buf(),
        }
    }

    fn list_packages(&self) -> Result<Vec<PackageSpec>, RegistryError> {
        let mut packages = Vec::new();
        let packages_dir = self.path.join("packages");
        let dir_entries = fs::read_dir(&packages_dir)
            .map_err(RegistryError::ReadError)?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(RegistryError::ReadError)?;

        for entry in dir_entries {
            let r#type = entry.file_type().map_err(|e| RegistryError::ReadError(e))?;

            if !r#type.is_dir() {
                return Err(RegistryError::FormatError {
                    message: format!(
                        "Invalid entry in registry: {} is not a directory",
                        entry.file_name().to_string_lossy()
                    ),
                    details: vec![],
                    source: None,
                });
            }

            let package_toml = entry.path().join("package.toml");

            let content = fs::read_to_string(&package_toml).map_err(RegistryError::ReadError)?;
            let pkg: PackageSpec =
                toml::from_str(&content).map_err(|e| RegistryError::FormatError {
                    message: format!(
                        "Failed to parse package.toml for {}",
                        entry.file_name().to_string_lossy()
                    ),
                    details: vec![e.to_string()],
                    source: Some(Box::new(e)),
                })?;
            packages.push(pkg);
        }
        Ok(packages)
    }
}

impl Registry for FileRegistry {
    fn search_packages(&self, term: String) -> Result<Vec<PackageSpec>, RegistryError> {
        let term_lower = term.to_lowercase();
        let mut results = Vec::new();
        for pkg in self.list_packages()? {
            if pkg.name.to_lowercase().contains(&term_lower)
                || pkg.description.to_lowercase().contains(&term_lower)
            {
                results.push(pkg);
            }
        }
        Ok(results)
    }

    fn list_versions(&self, package: &str) -> Result<Vec<String>, RegistryError> {
        let mut versions = Vec::new();
        let versions_dir = self.path.join("packages").join(package).join("versions");
        if versions_dir.exists() {
            for entry in fs::read_dir(versions_dir).map_err(RegistryError::ReadError)? {
                let entry = entry.map_err(RegistryError::ReadError)?;
                if !entry
                    .path()
                    .extension()
                    .map(|e| e == "toml")
                    .unwrap_or(false)
                {
                    return Err(RegistryError::FormatError {
                        message: format!(
                            "Invalid entry in versions directory for {}: {} is not a .toml file",
                            package,
                            entry.file_name().to_string_lossy()
                        ),
                        details: vec![],
                        source: None,
                    });
                }
                let content = fs::read_to_string(entry.path()).map_err(RegistryError::ReadError)?;
                let v: VersionSpec =
                    toml::from_str(&content).map_err(|e| RegistryError::FormatError {
                        message: format!(
                            "Failed to parse version spec for {}@{}",
                            package,
                            entry.file_name().to_string_lossy().replace(".toml", "")
                        ),
                        details: vec![],
                        source: Some(Box::new(e)),
                    })?;
                versions.push(v.version);
            }
        }
        Ok(versions)
    }

    fn get_version_spec(
        &self,
        package: &str,
        version: &str,
    ) -> Result<Option<VersionSpec>, RegistryError> {
        let version_toml = self
            .path
            .join("packages")
            .join(package)
            .join("versions")
            .join(format!("{}.toml", version));
        let content = fs::read_to_string(&version_toml).map_err(RegistryError::ReadError)?;
        toml::from_str(&content).map_err(|e| RegistryError::FormatError {
            message: format!("Failed to parse version spec for {}@{}", package, version),
            details: vec![e.to_string()],
            source: Some(Box::new(e)),
        })
    }
}
