use simplelog::{ColorChoice, ConfigBuilder, LevelFilter, TermLogger, TerminalMode};

pub fn init_logger(level: LevelFilter) {
    let mut builder = ConfigBuilder::new();
    builder
        .set_thread_level(LevelFilter::Off)
        .set_time_level(LevelFilter::Off);
    #[allow(clippy::unwrap_used)]
    TermLogger::init(
        level,
        builder.build(),
        TerminalMode::Stderr,
        ColorChoice::Auto,
    )
    .unwrap();
}
