use crate::{configs::config_types::*, core::runner::RampUpStrategy};
use clap::{Parser, Subcommand};

#[derive(Parser, Clone)]
pub struct Args {
    #[clap(short, long, default_value = "ws://localhost:9001")]
    pub url: String,

    #[command(subcommand)]
    pub runner: RunnerType,

    #[clap(short, long, value_enum, default_value_t = BehaviourType::NoChoice)]
    pub behavior: BehaviourType,

    // Connections number
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
    pub connection_number: u32,

    #[clap(long, default_value = "30", value_parser = clap::value_parser!(u64).range(1..))]
    pub connection_duration: u64,

    #[clap(short, long, default_value = "0", value_parser = clap::value_parser!(u64).range(0..))]
    pub connection_pause: u64,

    // Waves overall
    #[clap(long, default_value = "1", value_parser = clap::value_parser!(u32).range(1..))]
    pub waves_number: u32,

    #[arg(short, long, default_value = "0", value_parser = clap::value_parser!(u64).range(0..))]
    pub waves_pause: u64,

    // Spam amount
    #[arg(long, default_value = "100", value_parser = clap::value_parser!(u64).range(1..))]
    pub spam_pause: u64,
}

#[derive(Subcommand, Clone, Debug)]
pub enum RunnerType {
    /// No runner strategy
    NoChoice,

    /// Flat runner (no ramp-up)
    Flat,

    /// Ramp-up strategy (nested enum)
    RampUp {
        /// Ramp-up strategy type
        #[command(subcommand)]
        strategy: RampUpStrategyArgs,
    },

    /// Sine strategy
    Sine,
}

#[derive(Subcommand, Clone, Debug)]
pub enum RampUpStrategyArgs {
    /// Linear ramp-up strategy
    Linear {
        /// Target number of connections
        #[arg(long, default_value = "100", value_parser = clap::value_parser!(u32).range(1..))]
        target_connection: u32,

        /// Ramp-up duration in seconds
        #[arg(long, default_value = "60", value_parser = clap::value_parser!(u64).range(1..))]
        ramp_duration: u64,
    },

    /// Stepped ramp-up strategy
    Steps {
        /// Duration of each step in milliseconds
        #[arg(long, default_value = "1000", value_parser = clap::value_parser!(u32).range(1..))]
        step_duration: u32,

        /// Number of connections per step
        #[arg(long, default_value = "10", value_parser = clap::value_parser!(u32).range(1..))]
        step_size: u32,
    },

    /// Exponential ramp-up strategy
    Expo {
        /// Growth factor
        #[arg(long, default_value = "2", value_parser = clap::value_parser!(u32).range(1..))]
        growth_factor: u32,
        #[arg(long, default_value = "2000", value_parser = clap::value_parser!(u64).range(1..))]
        soaking_time: u64,
    },
}

impl Into<RampUpStrategy> for RampUpStrategyArgs {
    fn into(self) -> RampUpStrategy {
        match self {
            RampUpStrategyArgs::Linear {
                target_connection,
                ramp_duration,
            } => RampUpStrategy::Linear {
                target_connection,
                ramp_duration,
            },
            RampUpStrategyArgs::Steps {
                step_duration,
                step_size,
            } => RampUpStrategy::Stepped {
                step_duration,
                step_size,
            },
            RampUpStrategyArgs::Expo {
                growth_factor,
                soaking_time,
            } => RampUpStrategy::Expotential {
                growth_factor,
                soaking_time,
            },
        }
    }
}
