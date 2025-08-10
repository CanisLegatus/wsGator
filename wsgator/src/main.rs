use crate::configs::config_types::*;
use clap::Parser;
use core::behaviour::*;
use core::n_executor::NExecutor;
use core::runner::CommonRunnerConfig;
use core::runner::LinearRunner;
use core::runner::RampUpRunner;
use core::runner::Runner;
use std::sync::Arc;

mod configs;
mod core;
mod strategy_core;

use configs::cmd_args::*;
use configs::common_config::*;
use core::error_log::*;
use core::executor::*;
use strategy_core::{flat_strategy::*, flood_strategy::*, ramp_up_strategy::*, strategy::*};

type Factories = (
    Box<dyn Fn() -> Box<dyn Runner>>,
    Box<dyn Fn() -> Box<dyn Behaviour>>,
);

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
        AttackStrategyType::NoChoice => Arc::new(FlatStrategy {
            common_config: Arc::new(CommonConfig::from(args).with_external_timer()),
        }),
    }
}

pub fn get_factories(args: &Args) -> Factories {
    let common_runner_config = CommonRunnerConfig {
        url: args.url.clone(),
        connection_number: args.connection_number,
        connection_duration: args.connection_duration,
    };

    let runner_factory: Box<dyn Fn() -> Box<dyn Runner>> = {
        match args.strategy {
            AttackStrategyType::NoChoice => Box::new(move || {
                Box::new(LinearRunner {
                    common_config: common_runner_config.clone(),
                }) as Box<dyn Runner>
            }),
            AttackStrategyType::Flat => Box::new(move || {
                Box::new(LinearRunner {
                    common_config: common_runner_config.clone(),
                }) as Box<dyn Runner>
            }),
            AttackStrategyType::RampUp => Box::new(move || {
                Box::new(RampUpRunner {
                    common_config: common_runner_config.clone(),
                }) as Box<dyn Runner>
            }),
            AttackStrategyType::Flood => Box::new(move || {
                Box::new(LinearRunner {
                    common_config: common_runner_config.clone(),
                }) as Box<dyn Runner>
            }),
        }
    };

    let behaviour_factory = {
        match args.behavior {
            BehaviourType::NoChoice => || Box::new(PingPongBehaviour {}) as Box<dyn Behaviour>,
            BehaviourType::PingPong => || Box::new(PingPongBehaviour {}) as Box<dyn Behaviour>,
            BehaviourType::Silent => || Box::new(SilentBehaviour {}) as Box<dyn Behaviour>,
            BehaviourType::Flood => || Box::new(FloodBehaviour {}) as Box<dyn Behaviour>,
        }
    };

    (Box::new(runner_factory), Box::new(behaviour_factory))
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let error_log = ErrorLog::new();
    let strategy: Arc<dyn AttackStrategy + Send + Sync> = get_strategy(args.clone());

    let executor_task = tokio::spawn({
        let error_log = Arc::clone(&error_log);
        let strategy = Arc::clone(&strategy);

        async move {
            let executor = Executor;
            let _ = executor.run(strategy, Arc::clone(&error_log)).await;
        }
    });

    // New implementations
    let _executor = NExecutor::from_args(args);

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\n---> Interrupted by ctrl + c");
        }
        _ = executor_task => {
            println!("---> Executor finished normally...");
        }
    }

    println!("---> Logs:\n{error_log}");
}
