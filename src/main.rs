use clap::{Parser, Subcommand};
use fpm::{install_packages, remove_packages};

#[derive(Debug, Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Install {
        #[arg()]
        packages: Vec<String>,

        #[arg(long)]
        dest: Option<String>,
    },
    Remove {
        #[arg()]
        packages: Vec<String>,
    },
    List {
        #[arg(short, long)]
        installed: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Install { packages, dest } => {
            install_packages(packages, dest.clone()).await?;
        }
        Commands::Remove { packages } => {
            remove_packages(packages).await?;
        }
        Commands::List { installed } => {
            if *installed {
                todo!()
            } else {
                todo!()
            }
        }
    }

    Ok(())
}
