/// Settings for a single-worker app.
pub struct Config {
    pub name: String,
    pub workers: usize,
}

impl Config {
    /// Settings for an app named `name`, with one worker.
    pub fn new(name: &str) -> Self {
        Config { name: name.to_string(), workers: 1 }
    }
}

/// One line describing the app, for start-up logs.
pub fn describe(config: &Config) -> String {
    format!("{} with {} worker(s)", config.name, config.workers)
}
