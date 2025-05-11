use clap::Parser;
use std::sync::Arc;

mod strategies;
use strategies::*;

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value = "ws://localhost:9001")]
    url: String,

    #[clap(short, long, value_enum, default_value_t = AttackStrategyType::Flat)]
    strategy: AttackStrategyType,

    // Connections number
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
    connection_number: u32,

    #[clap(long, default_value = "30", value_parser = clap::value_parser!(u64).range(1..))]
    connection_duration: u64,

    #[clap(short, long, default_value = "20", value_parser = clap::value_parser!(u64).range(0..))]
    connection_pause: u64,

    // Waves overall
    #[clap(long, default_value = "1", value_parser = clap::value_parser!(u32).range(1..))]
    waves_number: u32,

    #[arg(short, long, default_value = "0", value_parser = clap::value_parser!(u64).range(0..))]
    waves_pause: u64,

    // Spam amount
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u64).range(1..))]
    spam_pause: u64,
}

fn get_strategy(strategy_type: AttackStrategyType) -> Arc<dyn AttackStrategy + Send + Sync> {
    match strategy_type {
        AttackStrategyType::Flat => Arc::new(FlatStrategy),
        AttackStrategyType::RampUp => Arc::new(RampUpStrategy),
        AttackStrategyType::Flood => Arc::new(FloodStrategy),
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let strategy: Arc<dyn AttackStrategy + Send + Sync> = get_strategy(args.strategy);
    let config: AttackConfig = args.into();

    strategy.run(&config).await;

    tokio::signal::ctrl_c().await.unwrap();
}
