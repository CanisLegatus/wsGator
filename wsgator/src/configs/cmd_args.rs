use crate::AttackStrategyType;
use clap::Parser;

#[derive(Parser)]
pub struct Args {
    #[clap(short, long, default_value = "ws://localhost:9001")]
    pub url: String,

    #[clap(short, long, value_enum, default_value_t = AttackStrategyType::Flat)]
    pub strategy: AttackStrategyType,

    // Connections number
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
    pub connection_number: u32,

    #[clap(long, default_value = "30", value_parser = clap::value_parser!(u64).range(1..))]
    pub connection_duration: u64,

    #[clap(short, long, default_value = "0", value_parser = clap::value_parser!(u64).range(0..))]
    pub connection_pause: u64,

    // Waves overall
    #[clap(long, default_value = "1", value_parser = clap::value_parser!(u32).range(1..))]
    pub waves_number: u32,

    #[arg(short, long, default_value = "0", value_parser = clap::value_parser!(u64).range(0..))]
    pub waves_pause: u64,

    // Spam amount
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u64).range(1..))]
    pub spam_pause: u64,
}
