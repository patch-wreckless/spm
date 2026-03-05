use std::{fs, path::Path, process::Command};

use sha2::Digest;

use crate::registry;

pub fn install(
    package: &str,
    spec: &registry::VersionSpec,
) -> Result<(), Box<dyn std::error::Error>> {
    let home =
        dirs::home_dir().ok_or::<Box<dyn std::error::Error>>("HOME directory not found.".into())?;
    let store = home.join(".spm/store").join(package).join(&spec.version);
    fs::create_dir_all(&store)?;

    let archive_path = store.join("source.tar");
    println!("Downloading {} …", spec.source.url);
    download_file(&spec.source.url, &archive_path)?;

    println!("Verifying SHA256 …");
    if !verify_sha256(&archive_path, &spec.source.sha256) {
        return Err("SHA256 mismatch".into());
    }

    if spec.signature.r#type != "gpg" {
        return Err(format!("Unsupported signature type: {}", spec.signature.r#type).into());
    }

    let sig_path = store.join("source.tar.sig");
    println!("Downloading signature {} …", spec.signature.url);
    download_file(&spec.signature.url, &sig_path)?;

    println!("Verifying GPG signature …");
    verify_gpg(&sig_path, &archive_path, &spec.signature.expected_keys)?;

    extract_tar_to_source(&archive_path, &store)?;

    let install_dir = store.join("install");
    fs::create_dir_all(&install_dir)?;

    let build_dir = store.join("build");
    fs::create_dir_all(&build_dir)?;

    let source_path = store.join("source");

    let home = dirs::home_dir().ok_or("HOME directory not found")?;
    let runtime = home.join(".spm/runtime");

    let path = std::env::var("PATH").unwrap_or_else(|_| String::new());

    let output = Command::new(format!("{}/configure", source_path.to_string_lossy()))
        .env(
            "PATH",
            format!("{}/bin:{}", runtime.to_string_lossy(), path),
        )
        .arg(format!("--prefix={}", install_dir.to_string_lossy()))
        .arg(format!(
            "PKG_CONFIG_PATH={}/lib/pkgconfig",
            runtime.to_string_lossy()
        ))
        .arg(format!("CPPFLAGS=-I{}/include", runtime.to_string_lossy()))
        .arg(format!(
            "LDFLAGS=-L{}/lib -Wl,-rpath,{}/lib",
            runtime.to_string_lossy(),
            runtime.to_string_lossy()
        ))
        .current_dir(&build_dir)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "./configure failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let parallelism = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let output = Command::new("make")
        .arg(format!("-j{}", parallelism))
        .current_dir(&build_dir)
        .output()?;
    if !output.status.success() {
        return Err(format!("make failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }

    let output = Command::new("make")
        .arg("install")
        .current_dir(&build_dir)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "make install failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    link_into_runtime(&install_dir)?;

    println!("Installed {}@{}", package, spec.version);
    Ok(())
}

fn download_file(url: &str, dest: &Path) -> std::io::Result<()> {
    let mut resp = reqwest::blocking::get(url).expect("Request failed");
    let mut out = fs::File::create(dest)?;
    std::io::copy(&mut resp, &mut out)?;
    Ok(())
}

fn verify_sha256(path: &Path, expected: &str) -> bool {
    let data = fs::read(path).expect("Cannot read file");
    let mut hasher = sha2::Sha256::new();
    hasher.update(&data);
    let hash = hasher.finalize();
    let hex = format!("{:x}", hash);
    hex == expected
}

fn verify_gpg(
    sig_path: &Path,
    archive_path: &Path,
    expected_keys: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("gpg")
        .arg("--status-fd=1")
        .arg("--verify")
        .arg(sig_path)
        .arg(archive_path)
        .output()?;

    if !output.status.success() {
        return Err("GPG signature verification failed".into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut valid_signers = Vec::new();

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("[GNUPG:] VALIDSIG ") {
            let fingerprint = rest.split_whitespace().next().unwrap_or("");
            valid_signers.push(fingerprint.to_uppercase());
        }
    }

    if valid_signers.is_empty() {
        return Err("No valid signatures found".into());
    }

    let expected: Vec<String> = expected_keys.iter().map(|k| k.to_uppercase()).collect();

    for key in &expected {
        if !valid_signers.iter().any(|f| f == key) {
            return Err(format!("Missing required signature from {}", key).into());
        }
    }

    Ok(())
}

fn extract_tar_to_source(
    archive_path: &Path,
    dest: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let extract = dest.join("extract");
    fs::create_dir_all(&extract)?;

    let output = Command::new("tar")
        .arg("-xf")
        .arg(archive_path)
        .arg("-C")
        .arg(&extract)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "tar extraction failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    // Some source archives contain a top-level directory, while others contain files directly. We
    // want a top-level directory called "source" either way.
    let top_dirs = fs::read_dir(&extract)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name())
        .collect::<Vec<_>>();

    let source_path = dest.join("source");

    if top_dirs.len() == 1 && extract.join(&top_dirs[0]).is_dir() {
        fs::rename(extract.join(&top_dirs[0]), &source_path)?;
    } else {
        fs::create_dir_all(&source_path)?;
        for entry in top_dirs {
            let from = extract.join(&entry);
            let to = source_path.join(&entry);
            fs::rename(from, to)?;
        }
    }
    fs::remove_dir_all(extract)?;
    Ok(())
}

fn link_into_runtime(install_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::symlink;

    let home = dirs::home_dir().ok_or("HOME directory not found")?;
    let runtime = home.join(".spm/runtime");

    fs::create_dir_all(&runtime)?;

    fn recurse(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let target = dst.join(entry.file_name());

            if path.is_dir() {
                fs::create_dir_all(&target)?;
                recurse(&path, &target)?;
            } else {
                if target.exists() {
                    fs::remove_file(&target)?;
                }

                symlink(&path, &target)?;
            }
        }

        Ok(())
    }

    recurse(install_dir, &runtime)?;

    Ok(())
}
