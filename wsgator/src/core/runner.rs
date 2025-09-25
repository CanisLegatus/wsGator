use crate::core::timer::TimerType;
use futures::future::join_all;
use std::pin::Pin;
use std::time::Duration;
use tokio_tungstenite::tungstenite::client;

use crate::Arc;
use crate::core::timer::Timer;
use async_trait::async_trait;
use futures::{FutureExt, StreamExt, stream};
use tokio::sync::watch::Sender as WatchSender;
use tokio::task::JoinHandle;

use super::behaviour::Behaviour;
use super::client_context::ClientContext;
use super::error::WsGatorError;
use super::monitor::Monitor;

// Runner
// Algorithm of a load
// Creation and management of connection pool
// Passing params to ClientContext

pub struct ClientBatch {
    clients: Vec<ClientContext>,
    stop_tx: Option<WatchSender<bool>>,
}

#[async_trait]
pub trait Runner: Send + Sync {
    fn get_common_config(&self) -> &CommonRunnerConfig;

    fn create_clients(&self, behaviour: Arc<dyn Behaviour>) -> ClientBatch {
        let common_config = self.get_common_config();

        let mut timer = Timer::new(TimerType::Outer);
        let stop_tx = timer.get_outer_timer();

        let clients: Vec<ClientContext> = (0..common_config.connection_number)
            .map(|id| {
                // Creating a client context here
                ClientContext::new(
                    id,
                    common_config.url.clone(),
                    timer.clone().into(),
                    behaviour.clone(),
                    Arc::new(Monitor {}),
                )
            })
            .collect();

        ClientBatch { clients, stop_tx }
    }

    // Function to manipulate start runners
    async fn run_clients(
        &self,
        client_batch: ClientBatch,
    ) -> Vec<JoinHandle<Result<(), WsGatorError>>> {
        let connection_duration = Duration::from_secs(self.get_common_config().connection_duration);

        // Spawning outside timer
        if let Some(stop_tx) = client_batch.stop_tx {
            tokio::spawn(async move {
                let _ = tokio::time::sleep(connection_duration).await;
                let _ = stop_tx.send(false);
            });
        }

        stream::iter(client_batch.clients)
            .map(|mut client| async move {
                tokio::spawn(async move { client.run().await.map_err(WsGatorError::from) })
            })
            .buffer_unordered(300)
            .collect()
            .await
    }

    // Function to create, run and collect final results from ClientContexts
    async fn run(&self, behaviour: Arc<dyn Behaviour>) {
        let client_batch = self.create_clients(behaviour);
        let join_handle_vec = self.run_clients(client_batch).await;
        // TODO: Get all handles and check them for errors
        join_all(join_handle_vec).await;
    }
}

// Structs
#[derive(Clone)]
pub struct CommonRunnerConfig {
    pub url: String,
    pub connection_number: u32,
    pub connection_duration: u64,
}

type RampUpRunnerFuture =
    Pin<Box<dyn Future<Output = Vec<JoinHandle<Result<(), WsGatorError>>>> + Send>>;

#[derive(Clone)]
pub enum RampUpStrategy {
    Linear {
        target_connection: u32,
        ramp_duration: u64,
    },
    Stepped {
        step_duration: u32,
        step_size: u32,
    },
    Expotential {
        growth_factor: u32,
        soaking_time: u64,
    },
    Sine {
        min_connections: u32,
        max_connections: u32,
        period: u32,
    },
}

impl RampUpStrategy {
    fn run(self, config: CommonRunnerConfig, client_batch: ClientBatch) -> RampUpRunnerFuture {
        match self {
            RampUpStrategy::Linear {
                target_connection,
                ramp_duration,
            } => self.get_linear(config, client_batch, target_connection, ramp_duration),
            RampUpStrategy::Stepped {
                step_duration,
                step_size,
            } => self.get_stepped(config, client_batch, step_duration, step_size),
            RampUpStrategy::Expotential {
                growth_factor,
                soaking_time,
            } => self.get_expotential(config, client_batch, growth_factor, soaking_time),
            RampUpStrategy::Sine {
                min_connections,
                max_connections,
                period,
            } => self.get_sine(),
        }
    }
    fn get_linear(
        self,
        config: CommonRunnerConfig,
        client_batch: ClientBatch,
        target_connection: u32,
        ramp_duration: u64,
    ) -> RampUpRunnerFuture {
        Box::pin(async move {
            let connection_duration = Duration::from_secs(config.connection_duration);
            let delay_millis =
                (config.connection_duration * 1000) / config.connection_number as u64;

            // Spawning outide timer
            if let Some(stop_tx) = client_batch.stop_tx {
                tokio::spawn(async move {
                    let _ = tokio::time::sleep(connection_duration).await;
                    let _ = stop_tx.send(false);
                });
            }

            let mut result_vec = vec![];

            for mut client in client_batch.clients {
                let handle =
                    tokio::spawn(async move { client.run().await.map_err(WsGatorError::from) });
                tokio::time::sleep(Duration::from_millis(delay_millis)).await;
                result_vec.push(handle);
            }

            result_vec
        })
    }

    fn get_stepped(
        self,
        config: CommonRunnerConfig,
        client_batch: ClientBatch,
        step_duration: u32,
        step_size: u32,
    ) -> RampUpRunnerFuture {
        Box::pin(async move {
            let connection_duration = Duration::from_secs(config.connection_duration);
            let step_duration = Duration::from_millis(step_duration as u64);

            // Spawning outide timer
            if let Some(stop_tx) = client_batch.stop_tx {
                tokio::spawn(async move {
                    let _ = tokio::time::sleep(connection_duration).await;
                    let _ = stop_tx.send(false);
                });
            }

            let mut counter = 0;
            let mut result_vec = vec![];

            for mut client in client_batch.clients {
                counter += 1;
                if counter <= step_size {
                    let handle =
                        tokio::spawn(async move { client.run().await.map_err(WsGatorError::from) });
                    result_vec.push(handle);
                } else {
                    tokio::time::sleep(step_duration).await;
                    counter = 1;
                    let handle =
                        tokio::spawn(async move { client.run().await.map_err(WsGatorError::from) });
                    result_vec.push(handle);
                }
            }

            result_vec
        })
    }

    fn get_expotential(
        self,
        config: CommonRunnerConfig,
        client_batch: ClientBatch,
        growth_factor: u32,
        soaking_time: u64,
    ) -> RampUpRunnerFuture {
        Box::pin(async move {
            let connection_duration = Duration::from_secs(config.connection_duration);
            let soaking_time = Duration::from_millis(soaking_time);

            let mut current_batch_size = 1;
            let mut remaining = client_batch.clients.into_iter();
            let mut batches = vec![];

            // Preparing batches to launch
            loop {
                let batch = remaining
                    .by_ref()
                    .take(current_batch_size)
                    .collect::<Vec<_>>();

                if !batch.is_empty() {
                    batches.push(batch);
                } else {
                    break;
                }

                current_batch_size *= growth_factor as usize;
            }

            // Spawning outide timer
            if let Some(stop_tx) = client_batch.stop_tx {
                tokio::spawn(async move {
                    let _ = tokio::time::sleep(connection_duration).await;
                    let _ = stop_tx.send(false);
                });
            }

            // Launching tasks from batch in parallel
            let mut result_vec = vec![];

            for batch in batches {
                let handles: Vec<JoinHandle<Result<(), WsGatorError>>> = stream::iter(batch)
                    .map(|mut client| {
                        tokio::spawn(async move { client.run().await.map_err(WsGatorError::from) })
                    })
                    .collect()
                    .await;

                result_vec.extend(handles);
                tokio::time::sleep(soaking_time).await;
            }

            result_vec
        })
    }

    // TODO: Implement RampUpRunner Sine
    fn get_sine(self) -> RampUpRunnerFuture {
        Box::pin(async move {
            let result_vec = vec![];
            result_vec
        })
    }
}

pub struct LinearRunner {
    pub common_config: CommonRunnerConfig,
}

pub struct RampUpRunner {
    pub strategy: RampUpStrategy,
    pub common_config: CommonRunnerConfig,
}

pub struct SineRunner {
    pub common_config: CommonRunnerConfig,
    min_connections: u32,
    max_connections: u32,
    period: u32,
}

impl SineRunner {}

impl RampUpRunner {}

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

    async fn run_clients(
        &self,
        client_batch: ClientBatch,
    ) -> Vec<JoinHandle<Result<(), WsGatorError>>> {
        let strategy = self.strategy.clone();
        strategy
            .run(self.get_common_config().clone(), client_batch)
            .await
    }
}

#[async_trait]
impl Runner for SineRunner {
    fn get_common_config(&self) -> &CommonRunnerConfig {
        &self.common_config
    }

    // TODO: Sine runner logic
    async fn run_clients(
        &self,
        client_batch: ClientBatch,
    ) -> Vec<JoinHandle<Result<(), WsGatorError>>> {



        vec![]
    }
}
