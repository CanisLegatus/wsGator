use crate::AttackStrategy;
use crate::CommonConfig;
use async_trait::async_trait;
use std::sync::Arc;

pub struct FlatStrategy {
    pub common_config: Arc<CommonConfig>,
}

#[async_trait]
impl AttackStrategy for FlatStrategy {
    fn get_common_config(&self) -> Arc<CommonConfig> {
        self.common_config.clone()
    }
}
