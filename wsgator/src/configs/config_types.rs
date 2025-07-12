#[derive(clap::ValueEnum, Clone, Copy)]
pub enum AttackStrategyType {
    NoChoice,
    Flat,
    RampUp,
    Flood,
}

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum RunnerType {
    NoChoice,
    Flat,
    LinearRampUp,
    StepsRampUp,
    ExpoRampUp,
}

#[derive(clap::ValueEnum, Clone, Copy)]
pub enum BehaviourType {
    NoChoice,
    Silent,
    PingPong,
    Flood,
}
