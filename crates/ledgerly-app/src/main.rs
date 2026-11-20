//! Ledgerly desktop app entry point.

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();
    ledgerly_app::shell::run()
}
