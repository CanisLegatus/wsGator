use std::time::Duration;

use crate::Arc;
use async_trait::async_trait;
use futures::{stream, StreamExt};
use tokio::sync::watch;
use tokio::sync::watch::{Receiver as WatchReceiver, Sender as WatchSender};
use tokio::task::JoinHandle;

use super::behaviour::Behaviour;
use super::client_context::ClientContext;
use super::error::WsGatorError;
use super::monitor::Monitor;

// Runner
// Algorithm of a load
// Creation and management of connection pool
// Passing params to ClientContext

#[async_trait]
pub trait Runner: Send + Sync {
    fn get_common_config(&self) -> &CommonRunnerConfig;

    // What we do here exactly? Hmm... Client context here is not a config! It's an active actor
    fn collect_clients(
        &self,
        behaviour: Box<dyn Behaviour>,
    ) -> (Vec<ClientContext>, WatchSender<bool>) {
        let common_config = self.get_common_config();
        let (stop_tx, stop_rx) = watch::channel(false);
        let behaviour = Arc::new(behaviour);

        let clients: Vec<ClientContext> = (0..common_config.connection_number)
            .map(|id| {
                // Creating a client context here
                ClientContext::new(
                    id,
                    common_config.url.clone(),
                    stop_rx.clone(),
                    behaviour.clone(),
                    Arc::new(Monitor {}),
                )
            })
            .collect();

        (clients, stop_tx)
    }

    // Function to manipulate start runners
    async fn run_clients(
        &self,
        clients: Vec<ClientContext>,
        stop_tx: WatchSender<bool>,
    ) -> Vec<JoinHandle<Result<(), WsGatorError>>> {
        // Starting stop task

        let connection_duration = Duration::from_secs(self.get_common_config().connection_duration);

        tokio::spawn(async move {
            let _ = tokio::time::sleep(connection_duration).await;
            let _ = stop_tx.send(false);
        });

        stream::iter(clients)
            .map(|mut client| async move {
                tokio::spawn(async move { client.run().await.map_err(WsGatorError::from) })
            })
            .buffer_unordered(300)
            .collect()
            .await
    }

    // Function to create, run and collect final results from ClientContexts
    async fn run(&self, behaviour: Box<dyn Behaviour>) {
        let (clients, stop_tx) = self.collect_clients(behaviour);
        let join_handle_vec = self.run_clients(clients, stop_tx).await;
    }
}

// Structs
#[derive(Clone)]
pub struct CommonRunnerConfig {
    pub url: String,
    pub connection_number: u32,
    pub connection_duration: u64,
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
