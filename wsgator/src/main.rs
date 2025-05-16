use std::sync::Arc;
use clap::Parser;

mod core;
mod strategy_core;
mod configs;

use configs::cmd_args::*;
use core::error_log::*;
use core::executor::*;
use strategy_core::strategies::*;



fn get_strategy(
    args: Args,
) -> Arc<dyn AttackStrategy + Send + Sync> {
    match args.strategy {
        AttackStrategyType::Flat => Arc::new(FlatStrategy {
            common_config: Arc::new(CommonConfig::from(args).with_external_timer()),
        }),
        AttackStrategyType::RampUp => Arc::new(RampUpStrategy {
            common_config: Arc::new(CommonConfig::from(args)),
        }),
        AttackStrategyType::Flood => Arc::new(FloodStrategy {
            spam_pause: args.spam_pause,
            common_config: Arc::new(CommonConfig::from(args).with_external_timer()),
        }),
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let error_log = ErrorLog::new();
    let strategy: Arc<dyn AttackStrategy + Send + Sync> = get_strategy(args);
    let executor = Executor;
    let _ = executor.run(strategy).await;

    tokio::signal::ctrl_c().await.unwrap();
}
