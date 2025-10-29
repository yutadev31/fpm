use clap::{Parser, Subcommand};
use fpm::{
    MakeOptions, check_dependencies, install_packages, make_packages, new_build_script,
    remove_packages, update_build_script,
};

#[derive(Debug, Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Script {
        #[clap(subcommand)]
        subcommand: EditScriptSubcommands,
    },
    Make(MakeOptions),
    Install {
        #[arg()]
        packages: Vec<String>,
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

#[derive(Debug, Subcommand)]
enum EditScriptSubcommands {
    New {
        #[arg()]
        package: String,
    },
    Update {
        #[arg()]
        package: String,

        #[arg()]
        version: String,
    },
    CheckDependencies,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Script { subcommand } => match subcommand {
            EditScriptSubcommands::New { package } => {
                new_build_script(package).await?;
            }
            EditScriptSubcommands::Update { package, version } => {
                update_build_script(package, version).await?;
            }
            EditScriptSubcommands::CheckDependencies => {
                check_dependencies().await?;
            }
        },
        Commands::Make(opts) => make_packages(opts.clone()).await?,
        Commands::Install { packages } => {
            install_packages(packages).await?;
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
