use async_trait::async_trait;

#[async_trait]
pub trait Behaviour: Send + Sync {
    async fn on_message(&self) {}
    async fn on_connect(&self) {}
    async fn on_error(&self) {}
    async fn on_start(&self) {}
    async fn on_stop(&self) {}
}

// Structs

pub struct SilentBehaviour {}

pub struct PingPongBehaviour {}

pub struct FloodBehaviour {}

// Implementations

#[async_trait]
impl Behaviour for SilentBehaviour {}

#[async_trait]
impl Behaviour for PingPongBehaviour {}

#[async_trait]
impl Behaviour for FloodBehaviour {}
