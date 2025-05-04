use clap::Parser;

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

fn get_strategy(strategy_type: AttackStrategyType) -> Box<dyn AttackStrategy + Send + Sync> {
    match strategy_type {
        AttackStrategyType::Flat => Box::new(FlatStrategy),
        AttackStrategyType::RampUp => Box::new(RampUpStrategy),
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let strategy = get_strategy(args.strategy);
    strategy.run(&args).await;

    tokio::signal::ctrl_c().await.unwrap();
}
