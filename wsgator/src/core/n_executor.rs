use crate::{configs::cmd_args::Args, get_factories};
use futures::StreamExt;
use crate::ConnectionTaskFuture;
use super::{behaviour::Behaviour, runner::Runner};
use futures::stream;
use tokio_tungstenite::tungstenite::Error as WsError;

pub struct NExecutor {
    waves_number: u32,
    connections_number: u32,
    runner: Box<dyn Fn() -> Box<dyn Runner>>,
    behaviour: Box<dyn Fn() -> Box<dyn Behaviour>>,
}
//  Executor
//  Starts the test
//  Uses waves 
//  Collects Errors
//  Control TimeBounds

impl NExecutor {

    pub fn from_args(args: Args) -> Self {
        let (runner, behaviour) = get_factories(&args);

        Self {
            waves_number: args.waves_number,
            connections_number: args.connection_number,
            runner,
            behaviour,
        }
    }

    pub async fn run(&self) {
       
        // Starting wave
        for wave in 0..self.waves_number {
            let connections_vec = self.prepare_connections();

        }
    }

    pub async fn prepare_connections(&self) {

        let connections: Vec<Result<ConnectionTaskFuture, WsError>> =
            stream::iter(0..self.connections_number)
                .map(|i| {
                    let stop_rx = stop_rx.clone();
                    let strat = self.clone();

                    async move {
                        // Returning future from strategy
                        Ok(strat.get_task(con.url_under_fire.clone(), stop_rx, i))
                    }
                })
                .buffer_unordered(1000)
                .collect::<Vec<Result<ConnectionTaskFuture, WsError>>>()
                .await;
       connections 
    }
}
