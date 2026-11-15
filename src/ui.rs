use indicatif::{ProgressBar, ProgressStyle};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static ACTIVE: OnceLock<Mutex<Option<ProgressBar>>> = OnceLock::new();

pub fn step(n: u32, total: u32, message: &str) {
    let slot = ACTIVE.get_or_init(|| Mutex::new(None));
    let Ok(mut active) = slot.lock() else { return };
    if let Some(previous) = active.take() {
        previous.finish_with_message(format!("  ✓ {}", previous.message()));
    }
    let bar = ProgressBar::new_spinner();
    bar.set_style(
        ProgressStyle::with_template("  {spinner:.cyan} [{prefix:.magenta}] {msg:.white}")
            .expect("valid progress template")
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    bar.set_prefix(format!("{n}/{total}"));
    bar.set_message(message.to_owned());
    bar.enable_steady_tick(Duration::from_millis(90));
    *active = Some(bar);
}

pub fn finish() {
    let Some(slot) = ACTIVE.get() else { return };
    let Ok(mut active) = slot.lock() else { return };
    if let Some(bar) = active.take() {
        bar.finish_with_message(format!("  ✓ {}", bar.message()));
    }
}
