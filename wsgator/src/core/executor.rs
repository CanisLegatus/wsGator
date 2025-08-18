use super::behaviour;
use super::{behaviour::Behaviour, runner::Runner};
use crate::ConnectionTaskFuture;
use crate::{configs::cmd_args::Args, get_factories};
use futures::StreamExt;
use futures::stream;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Error as WsError;

pub struct Executor {
    waves_number: u32,
    runner: Box<dyn Fn() -> Box<dyn Runner>>,
    behaviour: Box<dyn Fn() -> Box<dyn Behaviour>>,
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
        for wave in 0..self.waves_number {
            // Unpacking everything
            let behaviour = (*self.behaviour)();
            let runner = (*self.runner)();

            runner.run(behaviour).await;
        }
    }
}
