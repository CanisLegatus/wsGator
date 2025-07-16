use async_trait::async_trait;

#[async_trait]
pub trait Behaviour: Send + Sync {}

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
