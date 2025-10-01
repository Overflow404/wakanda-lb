use clap::{Parser, command};

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub(crate) struct CliArguments {
    #[arg(short, long)]
    pub port: u16,

    #[arg(short, long)]
    pub target_servers_base_url: String,
}

#[cfg(test)]
mod test {
    use clap::Parser;

    use crate::cli_arguments::CliArguments;

    #[test]
    fn test_cli_arguments_long_flags() {
        let args = CliArguments::parse_from([
            "load-balancer",
            "--port",
            "3000",
            "--target-servers-base-url",
            "http://localhost:9000",
        ]);

        assert_eq!(args.port, 3000);
        assert_eq!(args.target_servers_base_url, "http://localhost:9000");
    }

    #[test]
    fn test_cli_arguments_short_flags() {
        let args =
            CliArguments::parse_from(["load-balancer", "-p", "3000", "-t", "https://example.com"]);

        assert_eq!(args.port, 3000);
        assert_eq!(args.target_servers_base_url, "https://example.com");
    }
}
