use clap::{Parser, ValueEnum, command};

#[derive(ValueEnum, Debug, Clone, PartialEq)]
#[clap(rename_all = "kebab_case")]
pub enum RoutingPolicy {
    RoundRobin,
    Random,
}

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub(crate) struct CliArguments {
    #[arg(short, long)]
    pub port: u16,

    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ',')]
    pub target_servers: Vec<String>,

    #[clap(short, long, value_enum, default_value = "round-robin")]
    pub routing_policy: RoutingPolicy,
}

#[cfg(test)]
mod test {
    use clap::Parser;

    use crate::cli_arguments::{CliArguments, RoutingPolicy};

    #[test]
    fn test_cli_arguments_long_flags() {
        let args = CliArguments::parse_from([
            "load-balancer",
            "--port",
            "3000",
            "--target-servers",
            "http://localhost:9000,http://localhost:9001",
            "--routing-policy",
            "random",
        ]);

        assert_eq!(args.port, 3000);
        assert_eq!(
            args.target_servers,
            Vec::from(["http://localhost:9000", "http://localhost:9001"])
        );
        assert_eq!(args.routing_policy, RoutingPolicy::Random);
    }

    #[test]
    fn test_cli_arguments_short_flags() {
        let args = CliArguments::parse_from([
            "load-balancer",
            "-p",
            "3000",
            "-t",
            "http://localhost:9000,http://localhost:9001",
            "-r",
            "round-robin",
        ]);

        assert_eq!(args.port, 3000);
        assert_eq!(
            args.target_servers,
            Vec::from(["http://localhost:9000", "http://localhost:9001"])
        );
        assert_eq!(args.routing_policy, RoutingPolicy::RoundRobin);
    }

    #[test]
    fn routing_policy_should_default_to_round_robin() {
        let args = CliArguments::parse_from([
            "load-balancer",
            "-p",
            "3000",
            "-t",
            "http://localhost:9000,http://localhost:9001",
        ]);

        assert_eq!(args.port, 3000);
        assert_eq!(
            args.target_servers,
            Vec::from(["http://localhost:9000", "http://localhost:9001"])
        );
        assert_eq!(args.routing_policy, RoutingPolicy::RoundRobin);
    }
}
