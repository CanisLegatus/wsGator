use crate::AttackStrategy;
use crate::CommonConfig;
use async_trait::async_trait;
use std::sync::Arc;

pub struct RampUpStrategy {
    pub common_config: Arc<CommonConfig>,
}

#[async_trait]
impl AttackStrategy for RampUpStrategy {
    fn get_common_config(&self) -> Arc<CommonConfig> {
        self.common_config.clone()
    }
}
