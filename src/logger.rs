use fern::Dispatch;
use chrono::Local;
use std::fs::File;

pub fn setup(is_debug: bool, log_trace: bool) -> Result<(), anyhow::Error> {
    let log_level = if log_trace {
        log::LevelFilter::Trace
    } else {
        log::LevelFilter::Info
    };

    let mut logger = Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}:{} [{}:{}] -- {}",
                Local::now().format("%H:%M:%S%.3f"),
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                message
            ))
        })
        .level(log_level);

    if is_debug {
        logger = logger.chain(std::io::stdout());
    } else {
        let log_file_path = "output.log";
        let log_file = File::create(log_file_path)?;
        log_file.set_len(0)?; 

        let log_file = fern::log_file(log_file_path)?;
        logger = logger.chain(log_file);
    }

    logger.apply()?;
    Ok(())
}