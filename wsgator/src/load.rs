use crate::config::{Config, parse_duration};
use serde::Serialize;
use std::{fmt, time::Duration};

#[derive(Debug, Clone)]
pub struct LoadProfile(Profile);

#[derive(Debug, Clone)]
enum Profile {
    Ramp(Duration),
    Steps(Vec<(u32, Duration)>),
}

impl std::str::FromStr for LoadProfile {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if let Some(ramp) = value.trim().strip_prefix("ramp:") {
            return Ok(Self(Profile::Ramp(parse_duration(ramp.trim())?)));
        }

        let mut steps = Vec::new();
        let mut previous = 0;
        let mut total = Duration::ZERO;
        for step in value.split(',') {
            let (target, duration) = step.split_once('@').ok_or(
                "expected ramp:DURATION or CONNECTIONS@DURATION[,CONNECTIONS@DURATION...]",
            )?;
            let target: u32 = target.trim().parse().map_err(|_| "invalid step target")?;
            if target <= previous || target > 1_000_000 {
                return Err("step targets must increase and be between 1 and 1000000".into());
            }
            let duration = parse_duration(duration.trim())?;
            total += duration;
            if total > Duration::from_secs(7 * 24 * 3600) || steps.len() == 64 {
                return Err("a stepped profile supports up to 64 steps and 7 days total".into());
            }
            steps.push((target, duration));
            previous = target;
        }
        Ok(Self(Profile::Steps(steps)))
    }
}

impl fmt::Display for LoadProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Profile::Ramp(duration) => write!(f, "ramp:{}", humantime::format_duration(*duration)),
            Profile::Steps(steps) => {
                for (index, (target, duration)) in steps.iter().enumerate() {
                    if index > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{target}@{}", humantime::format_duration(*duration))?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseKind {
    Ramp,
    Hold,
    Step,
    Drain,
}

impl fmt::Display for PhaseKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ramp => "ramp",
            Self::Hold => "hold",
            Self::Step => "step",
            Self::Drain => "drain",
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Phase {
    pub kind: PhaseKind,
    pub start: Duration,
    pub duration: Duration,
    pub target: u32,
}

impl Phase {
    pub fn end(self) -> Duration {
        self.start + self.duration
    }

    pub fn admission_time(self, client: u32) -> Duration {
        if self.kind == PhaseKind::Ramp {
            // Client zero starts immediately. The final slot precedes the hold boundary.
            let nanos = self.duration.as_nanos() * u128::from(client) / u128::from(self.target);
            return self.start + Duration::from_nanos(nanos as u64);
        }
        self.start
    }
}

pub(crate) struct LoadPlan {
    pub phases: Vec<Phase>,
    pub duration: Duration,
    pub connections: u32,
}

impl LoadPlan {
    pub fn new(config: &Config) -> Self {
        let mut plan = Self {
            phases: Vec::new(),
            duration: Duration::ZERO,
            connections: config.connections,
        };
        match config.load_profile.as_ref().map(|profile| &profile.0) {
            None => plan.duration = config.duration,
            Some(Profile::Ramp(ramp)) => {
                plan.push(PhaseKind::Ramp, *ramp, config.connections);
                plan.push(PhaseKind::Hold, config.duration, config.connections);
            }
            Some(Profile::Steps(steps)) => {
                for &(target, duration) in steps {
                    plan.push(PhaseKind::Step, duration, target);
                }
            }
        }
        plan
    }

    fn push(&mut self, kind: PhaseKind, duration: Duration, target: u32) {
        self.phases.push(Phase {
            kind,
            start: self.duration,
            duration,
            target,
        });
        self.duration += duration;
        self.connections = target;
    }
}
