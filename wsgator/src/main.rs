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

    // Connections overalls
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
    connections: u32,

    #[clap(long, default_value = "30", value_parser = clap::value_parser!(u32).range(1..))]
    connection_duration: u32,

    #[clap(short, long, default_value = "20", value_parser = clap::value_parser!(u32).range(0..))]
    connection_pause: u32,

    // Waves overall
    #[clap(long, default_value = "1", value_parser = clap::value_parser!(u32).range(1..))]
    waves_amount: u32,

    #[arg(short, long, default_value = "0", value_parser = clap::value_parser!(u32).range(0..))]
    waves_pause: u32,

    // Spam amount
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
    spam_pause: u32,
}

fn get_strategy(strategy_type: AttackStrategyType) -> Arc<dyn AttackStrategy> {
    match strategy_type {
        AttackStrategyType::Flat => Arc::new(FlatStrategy),
        AttackStrategyType::RampUp => Arc::new(RampUpStrategy),
        AttackStrategyType::Flood => Arc::new(FloodStrategy),
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let strategy = get_strategy(args.strategy);
    strategy.run(&args).await;

    tokio::signal::ctrl_c().await.unwrap();
}
