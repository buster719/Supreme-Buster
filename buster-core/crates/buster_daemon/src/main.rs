use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use buster_daemon::web::{serve, WebConfig};
use buster_daemon::{BusterDaemon, DaemonConfig};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    match command {
        "tick" => {
            let mut config = parse_options(&args[1..])?;
            config.max_ticks = Some(1);
            let mut daemon = BusterDaemon::new(config).map_err(|error| error.to_string())?;
            let record = daemon.tick(0).map_err(|error| error.to_string())?;
            print_record(&record);
            Ok(())
        }
        "run" => {
            let config = parse_options(&args[1..])?;
            let mut daemon = BusterDaemon::new(config).map_err(|error| error.to_string())?;
            let records = daemon.run().map_err(|error| error.to_string())?;
            for record in records {
                print_record(&record);
            }
            Ok(())
        }
        "web" => {
            let config = parse_web_options(&args[1..])?;
            serve(config).map_err(|error| error.to_string())
        }
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn parse_options(args: &[String]) -> Result<DaemonConfig, String> {
    let mut config = DaemonConfig::new(env::current_dir().map_err(|error| error.to_string())?);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                config.root = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--root requires a path".to_string())?,
                );
            }
            "--ticks" => {
                index += 1;
                let ticks = args
                    .get(index)
                    .ok_or_else(|| "--ticks requires a positive integer".to_string())?
                    .parse()
                    .map_err(|_| "--ticks must be a positive integer".to_string())?;
                config.max_ticks = Some(ticks);
            }
            "--forever" => {
                config.max_ticks = None;
            }
            "--interval-ms" => {
                index += 1;
                let interval_ms = args
                    .get(index)
                    .ok_or_else(|| "--interval-ms requires a positive integer".to_string())?
                    .parse()
                    .map_err(|_| "--interval-ms must be a positive integer".to_string())?;
                config.tick_interval = Duration::from_millis(interval_ms);
            }
            "--max-context-bytes" => {
                index += 1;
                config.max_context_bytes = args
                    .get(index)
                    .ok_or_else(|| "--max-context-bytes requires a positive integer".to_string())?
                    .parse()
                    .map_err(|_| "--max-context-bytes must be a positive integer".to_string())?;
            }
            "--no-body-reports" => {
                config.write_body_reports = false;
            }
            "--execute-actions" => {
                config.execute_actions = true;
            }
            other => return Err(format!("unknown option `{other}`")),
        }
        index += 1;
    }
    Ok(config)
}

fn parse_web_options(args: &[String]) -> Result<WebConfig, String> {
    let mut web = WebConfig::new(env::current_dir().map_err(|error| error.to_string())?);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                web.daemon.root = PathBuf::from(
                    args.get(index)
                        .ok_or_else(|| "--root requires a path".to_string())?,
                );
            }
            "--port" => {
                index += 1;
                web.port = args
                    .get(index)
                    .ok_or_else(|| "--port requires a port".to_string())?
                    .parse()
                    .map_err(|_| "--port must be a number".to_string())?;
            }
            "--host" => {
                index += 1;
                web.host = args
                    .get(index)
                    .ok_or_else(|| "--host requires an address".to_string())?
                    .to_string();
            }
            "--interval-ms" => {
                index += 1;
                let interval_ms = args
                    .get(index)
                    .ok_or_else(|| "--interval-ms requires a positive integer".to_string())?
                    .parse()
                    .map_err(|_| "--interval-ms must be a positive integer".to_string())?;
                web.daemon.tick_interval = Duration::from_millis(interval_ms);
            }
            "--max-context-bytes" => {
                index += 1;
                web.daemon.max_context_bytes = args
                    .get(index)
                    .ok_or_else(|| "--max-context-bytes requires a positive integer".to_string())?
                    .parse()
                    .map_err(|_| "--max-context-bytes must be a positive integer".to_string())?;
            }
            "--execute-actions" => {
                web.daemon.execute_actions = true;
            }
            "--no-open" => {
                web.open_browser = false;
            }
            "--no-body-reports" => {
                web.daemon.write_body_reports = false;
            }
            other => return Err(format!("unknown web option `{other}`")),
        }
        index += 1;
    }
    Ok(web)
}

fn print_record(record: &buster_daemon::DaemonTickRecord) {
    println!(
        "tick={} body={} action={} score={:.3} level={} reason={}",
        record.tick_index,
        record.body_mode,
        record.selected_action_kind,
        record.value_score,
        record.governance_level,
        record.reason
    );
}

fn print_usage() {
    println!(
        "usage:\n  busterd tick [--root PATH] [--execute-actions] [--max-context-bytes N] [--no-body-reports]\n  busterd run [--root PATH] [--ticks N|--forever] [--interval-ms N] [--execute-actions] [--max-context-bytes N] [--no-body-reports]\n  busterd web [--root PATH] [--port N] [--execute-actions] [--no-open]"
    );
}
