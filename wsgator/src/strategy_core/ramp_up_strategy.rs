use crate::AttackStrategy;
use crate::CommonConfig;
use crate::ErrorLog;
use crate::TasksRunner;
use crate::TasksVector;
use crate::TimerTask;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::task::JoinSet;

pub struct RampUpStrategy {
    pub common_config: Arc<CommonConfig>,
}

#[async_trait]
impl AttackStrategy for RampUpStrategy {
    fn get_common_config(&self) -> Arc<CommonConfig> {
        self.common_config.clone()
    }

    fn get_runner(
        &self,
        tasks: TasksVector,
        timer: TimerTask,
        config: Arc<CommonConfig>,
        log: Arc<ErrorLog>,
    ) -> TasksRunner {
        Box::pin(async move {
            let mut join_set = JoinSet::new();

            for task in tasks {
                match task {
                    Ok(task) => {
                        join_set.spawn(task);
                    }
                    Err(e) => {
                        log.count(e.into());
                    }
                }
            }

            Ok(join_set)
        })
    }
}
