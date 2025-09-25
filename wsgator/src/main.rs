use crate::configs::config_types::*;
use clap::Parser;
use core::behaviour::*;
use core::executor::Executor;
use core::runner::CommonRunnerConfig;
use core::runner::LinearRunner;
use core::runner::RampUpRunner;
use core::runner::Runner;
use std::sync::Arc;

mod configs;
mod core;

use configs::cmd_args::*;
use core::error_log::*;

type Factories = (
    Box<dyn Fn() -> Box<dyn Runner>>,
    Box<dyn Fn() -> Arc<dyn Behaviour>>,
);

pub fn get_factories(args: &Args) -> Factories {
    let common_runner_config = CommonRunnerConfig {
        url: args.url.clone(),
        connection_number: args.connection_number,
        connection_duration: args.connection_duration,
    };

    let runner_factory: Box<dyn Fn() -> Box<dyn Runner>> = {
        match &args.runner {
            RunnerType::NoChoice => Box::new(move || {
                Box::new(LinearRunner {
                    common_config: common_runner_config.clone(),
                }) as Box<dyn Runner>
            }),
            RunnerType::Flat => Box::new(move || {
                Box::new(LinearRunner {
                    common_config: common_runner_config.clone(),
                }) as Box<dyn Runner>
            }),
            RunnerType::RampUp { strategy } => {
                let strategy = strategy.clone();

                Box::new(move || {
                    Box::new(RampUpRunner {
                        strategy: strategy.clone().into(),
                        common_config: common_runner_config.clone(),
                    })
                })
            },
            RunnerType::Sine => {
                Box::new(move || {
                    Box::new(SineRunner)
                })
            }
        }
    };

    let behaviour_factory = {
        match args.behavior {
            BehaviourType::NoChoice => || Arc::new(DefaultBehaviour {}) as Arc<dyn Behaviour>,
            BehaviourType::PingPong => || Arc::new(PingPongBehaviour {}) as Arc<dyn Behaviour>,
            BehaviourType::Silent => || Arc::new(SilentBehaviour {}) as Arc<dyn Behaviour>,
            BehaviourType::Flood => || Arc::new(FloodBehaviour {}) as Arc<dyn Behaviour>,
        }
    };

    (Box::new(runner_factory), Box::new(behaviour_factory))
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let error_log = ErrorLog::new();

    let executor = Executor::from_args(args);

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\n---> Interrupted by ctrl + c");
        }
        _ = executor.run() => {
            println!("---> Executor finished normally...");
        }
    }

    println!("---> Logs:\n{error_log}");
}
