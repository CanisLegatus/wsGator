use crate::Args;

#[derive(Clone)]
pub struct CommonConfig {
    pub url_under_fire: String,
    pub connection_number: u32,
    pub connection_duration: u64,
    pub connection_pause: u64,
    pub waves_number: u32,
    pub waves_pause: u64,
    pub external_timer: bool,
}

impl From<Args> for CommonConfig {
    fn from(value: Args) -> Self {
        Self {
            url_under_fire: value.url.clone(),
            connection_number: value.connection_number,
            connection_duration: value.connection_duration,
            connection_pause: value.connection_pause,
            waves_number: value.waves_number,
            waves_pause: value.waves_pause,
            external_timer: false,
        }
    }
}

impl CommonConfig {
    pub fn with_external_timer(mut self) -> Self {
        self.external_timer = true;
        self
    }
}
