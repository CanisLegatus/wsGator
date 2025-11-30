use super::{behaviour::Behaviour, runner::Runner};
use crate::Arc;
use crate::{configs::cmd_args::Args, get_factories};

/// Executor is an high-level struct to create and manage runners and waves
pub struct Executor {
    waves_number: u32,
    runner: Box<dyn Fn() -> Box<dyn Runner>>,
    behaviour: Box<dyn Fn() -> Arc<dyn Behaviour>>,
}
//  Executor
//  Starts the test
//  Uses waves
//  Collects Errors
//  Control TimeBounds

impl Executor {
    pub fn from_args(args: Args) -> Self {
        let (runner, behaviour) = get_factories(&args);

        Self {
            waves_number: args.waves_number,
            runner,
            behaviour,
        }
    }

    pub async fn run(&self) {
        // Starting wave
        for _wave in 0..self.waves_number {
            // Unpacking everything
            let behaviour = (*self.behaviour)();
            let runner = (*self.runner)();

            runner.run(behaviour).await;
        }
    }
}
