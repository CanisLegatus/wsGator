#[derive(clap::ValueEnum, Clone, Copy)]
pub enum AttackStrategyType {
    NoChoice,
    Flat,
    RampUp,
    Flood,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum BehaviourType {
    NoChoice,
    Silent,
    PingPong,
    Flood,
}
