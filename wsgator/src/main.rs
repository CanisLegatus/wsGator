use clap::{Arg, ArgAction, Command, CommandFactory, FromArgMatches};
use std::{
    io::{self, Write},
    path::Path,
    process::ExitCode,
};
use tokio_util::sync::CancellationToken;
use wsgator::config::Config;

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args_os().len() == 1 {
        return interactive(false, None).await;
    }
    let matches = Config::command()
        .subcommand(
            Command::new("tui")
                .about("Open the persistent interactive control deck")
                .arg(
                    Arg::new("no-profile")
                        .long("no-profile")
                        .action(ArgAction::SetTrue)
                        .help("Do not read or write the local settings profile"),
                ),
        )
        .get_matches();
    if let Some(("tui", args)) = matches.subcommand() {
        return interactive(args.get_flag("no-profile"), Some(&matches)).await;
    }
    let config = Config::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
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

async fn interactive(no_profile: bool, matches: Option<&clap::ArgMatches>) -> ExitCode {
    match wsgator::tui::run(no_profile, matches).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("wsGator: {error}");
            ExitCode::from(2)
        }
    }
}
