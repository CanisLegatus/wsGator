use crate::Arc;
use async_trait::async_trait;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

use super::behaviour::SilentBehaviour;
use super::client_context::ClientContext;
use super::monitor::Monitor;

// Runner
// Algorithm of a load
// Creation and management of connection pool
// Passing params to ClientContext

#[async_trait]
pub trait Runner: Send + Sync {
    fn get_common_config(&self) -> &CommonRunnerConfig;

    fn collect_clients(&self) -> Vec<ClientContext> {
        let common_config = self.get_common_config();
        let (stop_tx, stop_rx) = watch::channel(false);

        // TODO Add behaviour here... (how to pass it ideomatically?)

        let connections: Vec<ClientContext> = (0..common_config.connection_number)
            .map(|i| {
                // Creating a client context here
                let (writer_tx, writer_rx) = mpsc::channel::<Message>(128);
                let client_context = ClientContext::new(
                    common_config.url.clone(),
                    i,
                    writer_tx,
                    stop_rx.clone(),
                    Arc::new(SilentBehaviour {}),
                    Arc::new(Monitor {}),
                );

                client_context
            })
            .collect();

        connections
    }
    async fn run(&self) {
        let clients = self.collect_clients();
    }
    fn create_task(&self, url: String, stop_rx: WatchReceiver<bool>, i: u32) {}
}

// Structs
#[derive(Clone)]
pub struct CommonRunnerConfig {
    pub url: String,
    pub connection_number: u32,
}

pub struct LinearRunner {
    pub common_config: CommonRunnerConfig,
}

pub struct RampUpRunner {
    pub common_config: CommonRunnerConfig,
}

// Implementations

#[async_trait]
impl Runner for LinearRunner {
    fn get_common_config(&self) -> &CommonRunnerConfig {
        &self.common_config
    }
}

#[async_trait]
impl Runner for RampUpRunner {
    fn get_common_config(&self) -> &CommonRunnerConfig {
        &self.common_config
    }
}
// Getting run logic
//            let runner = strategy
//              .clone()
//            .prepare_strategy(config.clone(), log.clone())
//           .await?;
//
//          println!("---> Wave ATTACK stage...");
//
// Running tasks and collecting them to join_set
//          let mut join_set = runner.await?;

//        // Handling errors in async tasks
//      while let Some(result_of_async_task) = join_set.join_next().await {
//        match result_of_async_task {
//          Ok(Ok(())) => continue,
//         Ok(Err(e)) => {
//           log.count(e);
//     }
//   Err(join_error) => {
//     println!("Join Error! Error: {join_error}");
//}
