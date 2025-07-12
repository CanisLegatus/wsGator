use super::{behaviour::Behaviour, runner::Runner};

pub struct NExecutor {
    runner: Box<dyn Fn() -> Box<dyn Runner>>,
    behaviour: Box<dyn Fn() -> Box<dyn Behaviour>>,
}

impl NExecutor {
    pub fn new(
        runner: Box<dyn Fn() -> Box<dyn Runner>>,
        behaviour: Box<dyn Fn() -> Box<dyn Behaviour>>,
    ) -> Self {
        Self { behaviour, runner }
    }
}
