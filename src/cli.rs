use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "spm", version = "0.1", about = "spm - Source Package Manager")]
pub struct Cli {
    #[arg(long, value_parser = parse_key_val, number_of_values = 1)]
    pub conf: Vec<(String, String)>,

    #[command(subcommand)]
    pub command: Commands,
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let (k, v) = s.split_once('=').ok_or("Expected KEY=VALUE")?;
    Ok((k.to_string(), v.to_string()))
}

#[derive(Subcommand)]
pub enum Commands {
    /// Search packages in the registry
    Search {
        /// Search term
        term: String,
    },

    /// List versions for a package
    Versions {
        /// Package name
        package: String,
    },

    /// Install a package version
    Install {
        /// Package name
        package: String,
        /// Version
        version: String,
    },
}
