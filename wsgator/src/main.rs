use clap::Parser;
use core::error_log;
use std::sync::Arc;

mod configs;
mod core;
mod strategy_core;

use configs::cmd_args::*;
use configs::common_config::*;
use core::error_log::*;
use core::executor::*;
use strategy_core::{flat_strategy::*, flood_strategy::*, ramp_up_strategy::*, strategy::*};

fn get_strategy(args: Args) -> Arc<dyn AttackStrategy + Send + Sync> {
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


    let executor_task = tokio::spawn({
        
        let error_log = Arc::clone(&error_log);
        let strategy = Arc::clone(&strategy);

        async move {
            let executor = Executor;
            let _ = executor.run(strategy, Arc::clone(&error_log)).await;
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = executor_task => {}
    }

    println!("Finished working!\nErrors: {:?}", error_log);
}
