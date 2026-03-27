mod cli;

use std::process;

use crate::cli::{
    args::parse_env_args,
    session::{format_stream_error, spawn_stop_listener, wait_for_session_end},
    source_resolver::build_config,
    summary::{encoder_summary_line, startup_diagnostic_note_lines, startup_summary_lines},
};
use raspi_stream::CameraStreamer;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let options = parse_env_args()?;
    let resolved = build_config(&options)?;

    println!("starting stream");
    let streamer = CameraStreamer::new(resolved.config);
    let session = streamer.start().map_err(format_stream_error)?;

    for line in startup_summary_lines(&options, streamer.config(), &resolved.source_label) {
        println!("{line}");
    }
    println!("{}", encoder_summary_line(&session.pipeline_label()));
    let startup_diagnostics = session.startup_diagnostic_entries();
    let startup_notes = if options.verbose {
        startup_diagnostics
            .into_iter()
            .map(|diagnostic| format!("  note      : {}", diagnostic.verbose_line()))
            .collect::<Vec<_>>()
    } else {
        startup_diagnostic_note_lines(&startup_diagnostics)
    };

    for line in startup_notes {
        println!("{line}");
    }
    println!("press Enter to stop");

    let stop_rx = spawn_stop_listener();
    wait_for_session_end(&session, &stop_rx)?;

    session.stop().map_err(format_stream_error)?;
    println!("stream stopped");

    Ok(())
}
