use clap::Parser;
use std::{
    io::{self, Write},
    path::Path,
    process::ExitCode,
};
use tokio_util::sync::CancellationToken;
use wsgator::config::Config;

#[tokio::main]
async fn main() -> ExitCode {
    let config = Config::parse();
    let json_path = config.json.clone();
    let stop = CancellationToken::new();
    let run = wsgator::run(config, stop.clone());
    tokio::pin!(run);
    let result = tokio::select! {
        result = &mut run => result,
        signal = tokio::signal::ctrl_c() => {
            if let Err(e) = signal { eprintln!("Cannot listen for Ctrl+C: {e}"); }
            stop.cancel();
            eprintln!("Stopping clients and collecting the report…");
            run.await
        }
    };
    let report = match result {
        Ok(report) => report,
        Err(e) => {
            eprintln!("wsGator: {e}");
            return ExitCode::from(2);
        }
    };
    let summary = report.summary();
    if json_path.as_deref() == Some(Path::new("-")) {
        eprint!("{summary}");
    } else if let Err(e) = io::stdout().lock().write_all(summary.as_bytes()) {
        eprintln!("Cannot write summary: {e}");
        return ExitCode::from(2);
    }
    if let Some(path) = json_path {
        let mut json = serde_json::to_vec_pretty(&report).expect("report serializes");
        json.push(b'\n');
        let result = if path == Path::new("-") {
            io::stdout().lock().write_all(&json)
        } else {
            std::fs::write(&path, json)
        };
        if let Err(e) = result {
            eprintln!("Cannot write JSON report: {e}");
            return ExitCode::from(2);
        }
    }
    if report.interrupted {
        ExitCode::from(130)
    } else if report.has_errors() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
