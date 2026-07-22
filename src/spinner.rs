use indicatif::{ProgressBar, ProgressStyle};

/// Run a closure while showing a spinner on stderr.
pub fn with_spinner<T>(message: &str, f: impl FnOnce() -> T) -> T {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    let result = f();
    pb.finish_and_clear();
    result
}
