use clap::ValueEnum;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt::writer::BoxMakeWriter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum LogTarget {
    Auto,
    Stdout,
    Stderr,
}

impl LogLevel {
    fn as_level_filter(self) -> LevelFilter {
        match self {
            LogLevel::Trace => LevelFilter::TRACE,
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
            LogLevel::Warn => LevelFilter::WARN,
            LogLevel::Error => LevelFilter::ERROR,
            LogLevel::Off => LevelFilter::OFF,
        }
    }
}

pub(crate) fn init_logging(level: LogLevel, target: LogTarget) {
    let writer = match target {
        LogTarget::Auto | LogTarget::Stderr => BoxMakeWriter::new(std::io::stderr),
        LogTarget::Stdout => BoxMakeWriter::new(std::io::stdout),
    };

    let _ = tracing_subscriber::fmt()
        .with_max_level(level.as_level_filter())
        .with_writer(writer)
        .with_ansi(false)
        .without_time()
        .try_init();
}
