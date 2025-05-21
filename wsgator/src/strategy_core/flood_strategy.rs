use crate::AttackStrategy;
use crate::CommonConfig;
use async_trait::async_trait;
use std::sync::Arc;

pub struct FloodStrategy {
    pub common_config: Arc<CommonConfig>,
    pub spam_pause: u64,
}

#[async_trait]
impl AttackStrategy for FloodStrategy {
    fn get_common_config(&self) -> Arc<CommonConfig> {
        self.common_config.clone()
    }
}
