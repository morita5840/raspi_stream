use std::{io, sync::mpsc, thread, time::Duration};

use raspi_stream::{StreamError, StreamEvent, StreamSession};

pub(crate) fn spawn_stop_listener() -> mpsc::Receiver<()> {
    let (stop_tx, stop_rx) = mpsc::channel();

    thread::spawn(move || {
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
        let _ = stop_tx.send(());
    });

    stop_rx
}

pub(crate) fn wait_for_session_end(
    session: &StreamSession,
    stop_rx: &mpsc::Receiver<()>,
) -> Result<(), String> {
    loop {
        if stop_rx.try_recv().is_ok() {
            return Ok(());
        }

        match session.poll_event(Duration::from_millis(200)) {
            Some(StreamEvent::Started { stream_url }) => {
                println!("stream ready: {stream_url}");
            }
            Some(StreamEvent::Warning { source, message }) => {
                eprintln!("warning: {source}: {message}");
            }
            Some(StreamEvent::Error { source, message }) => {
                let _ = session.stop();
                return Err(format!("stream error: {source}: {message}"));
            }
            Some(StreamEvent::Stopped { reason }) => {
                if let Some(reason) = reason {
                    eprintln!("stream stopped: {reason}");
                }
                return Ok(());
            }
            Some(StreamEvent::EndOfStream) => {
                return Ok(());
            }
            None => {}
        }
    }
}

pub(crate) fn format_stream_error(error: StreamError) -> String {
    match error {
        StreamError::InvalidConfig(message)
        | StreamError::InitFailed(message)
        | StreamError::PipelineBuildFailed(message)
        | StreamError::StateChangeFailed(message)
        | StreamError::RuntimeError(message) => message,
    }
}
