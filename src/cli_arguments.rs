use clap::{Parser, command};

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub(crate) struct CliArguments {
    #[arg(short, long)]
    pub port: u16,

    #[arg(short, long)]
    pub target_servers_base_url: String,
}
