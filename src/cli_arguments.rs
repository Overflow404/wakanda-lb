use clap::{Parser, ValueEnum, command};

#[derive(ValueEnum, Debug, Clone, PartialEq)]
#[clap(rename_all = "kebab_case")]
pub(crate) enum RoutingPolicy {
    RoundRobin,
    Random,
}

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub(crate) struct CliArguments {
    #[arg(short, long, default_value = "3000")]
    pub(crate) port: u16,

    #[clap(short, long, value_parser, num_args = 1.., value_delimiter = ',')]
    pub(crate) target_servers: Vec<String>,

    #[clap(short, long, value_enum, default_value = "round-robin")]
    pub(crate) routing_policy: RoutingPolicy,

    #[arg(long, default_value = "/health")]
    pub(crate) target_servers_health_path: String,

    #[arg(long, default_value = "10")]
    pub(crate) health_checker_polling_seconds: u64,
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
            "--target-servers-health-path",
            "/ready",
            "--health-checker-polling-seconds",
            "10",
        ]);

        assert_eq!(args.port, 3000);
        assert_eq!(
            args.target_servers,
            Vec::from(["http://localhost:9000", "http://localhost:9001"])
        );
        assert_eq!(args.routing_policy, RoutingPolicy::Random);
        assert_eq!(args.target_servers_health_path, "/ready");
        assert_eq!(args.health_checker_polling_seconds, 10);
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
            "-t",
            "http://localhost:9000,http://localhost:9001",
        ]);

        assert_eq!(args.routing_policy, RoutingPolicy::RoundRobin);
    }

    #[test]
    fn target_servers_health_path_should_default_to_health() {
        let args = CliArguments::parse_from([
            "load-balancer",
            "-t",
            "http://localhost:9000,http://localhost:9001",
        ]);

        assert_eq!(args.target_servers_health_path, "/health");
    }

    #[test]
    fn health_checker_polling_seconds_should_default_to_10() {
        let args = CliArguments::parse_from([
            "load-balancer",
            "-t",
            "http://localhost:9000,http://localhost:9001",
        ]);

        assert_eq!(args.health_checker_polling_seconds, 10);
    }

    #[test]
    fn port_should_default_to_3000() {
        let args = CliArguments::parse_from([
            "load-balancer",
            "-t",
            "http://localhost:9000,http://localhost:9001",
        ]);

        assert_eq!(args.port, 3000);
    }
}
