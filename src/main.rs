use std::path::Path;

use clap::Parser;

mod cli;
use cli::Commands;

mod conf;
mod registry;
use registry::Registry;

mod install;
use install::install;

fn main() {
    if let Err(e) = run() {
        eprintln!("fatal: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = cli::Cli::try_parse()?;
    let config = conf::load_config(&cli.conf)?;

    // todo: support multiple registry types (e.g. HTTP)
    let file_path = config
        .registry
        .url
        .strip_prefix("file://")
        .ok_or("Only file:// registry URLs are supported")?;

    let registry = registry::FileRegistry::new(Path::new(file_path));

    match cli.command {
        Commands::Search { term } => {
            let packages = registry.search_packages(term)?;
            if packages.is_empty() {
                println!("No packages found");
            } else {
                for pkg in packages {
                    println!("{} — {}", pkg.name, pkg.description);
                }
            }
        }
        Commands::Versions { package } => {
            let versions = registry.list_versions(&package)?;
            if versions.is_empty() {
                println!("No versions found");
            } else {
                println!("Available versions for {}:", package);
                for v in versions {
                    println!("  {}", v);
                }
            }
        }
        Commands::Install { package, version } => {
            let spec = registry
                .get_version_spec(&package, &version)?
                .ok_or_else(|| format!("Package {}@{} not found", package, version))?;
            install(&package, &spec)?;
        }
    }
    Ok(())
}
