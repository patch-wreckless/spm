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

    let status = Command::new("tar")
        .arg("-xf")
        .arg(archive_path)
        .arg("-C")
        .arg(&extract)
        .status()?;
    if !status.success() {
        return Err("tar extraction failed".into());
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
