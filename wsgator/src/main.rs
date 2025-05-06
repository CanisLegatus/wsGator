use clap::Parser;
use std::sync::Arc;

mod strategies;
use strategies::*;

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value = "ws://localhost:9001")]
    url: String,

    #[clap(short, long, default_value = "2")]
    connections: usize,

    #[clap(short, long, value_enum, default_value_t = AttackStrategyType::Flat)]
    strategy: AttackStrategyType,
}

fn get_strategy(strategy_type: AttackStrategyType) -> Arc<dyn AttackStrategy> {
    match strategy_type {
        AttackStrategyType::Flat => Arc::new(FlatStrategy),
        AttackStrategyType::RampUp => Arc::new(RampUpStrategy),
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let strategy = Arc::new(get_strategy(args.strategy));

    Arc::clone(&strategy).run(&args).await;

    tokio::signal::ctrl_c().await.unwrap();
}
