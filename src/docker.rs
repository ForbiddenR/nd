use std::{
    io::Read,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use anyhow::{Context, Result, bail};

use crate::app::{Container, Image};

#[derive(Debug)]
pub enum ProgressEvent {
    Line(String),
    Finished { success: bool, message: String },
}

pub fn list_containers() -> Result<Vec<Container>> {
    let output = run_docker(&[
        "ps",
        "-a",
        "--format",
        "{{.ID}}\t{{.Image}}\t{{.Command}}\t{{.Status}}\t{{.Names}}",
    ])?;

    Ok(output.lines().filter_map(parse_container).collect())
}

pub fn list_images() -> Result<Vec<Image>> {
    let output = run_docker(&[
        "images",
        "--format",
        "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.Size}}",
    ])?;

    Ok(output.lines().filter_map(parse_image).collect())
}

pub fn start_container(id: &str) -> Result<String> {
    run_docker(&["start", id])
}

pub fn stop_container(id: &str) -> Result<String> {
    run_docker(&["stop", id])
}

pub fn restart_container(id: &str) -> Result<String> {
    run_docker(&["restart", id])
}

pub fn remove_container(id: &str) -> Result<String> {
    run_docker(&["rm", id])
}

pub fn push_image_stream(repository: String, tag: String) -> Result<Receiver<ProgressEvent>> {
    let image = format!("{repository}:{tag}");
    let mut command = Command::new("nerdctl");
    command.args(["push", image.as_str()]);

    spawn_docker_stream(
        command,
        "failed to start nerdctl push",
        "push completed successfully",
        "push",
    )
}

pub fn remove_image(repository: &str, tag: Option<&str>) -> Result<String> {
    run_docker(&[
        "rmi",
        &format!("{}:{}", repository, tag.unwrap_or("latest")),
    ])
}

pub fn prune_builder() -> Result<String> {
    run_docker(&["builder", "prune", "-f"])
}

pub fn prune_images() -> Result<String> {
    run_docker(&["image", "prune", "-f"])
}

pub fn system_prune() -> Result<String> {
    run_docker(&["system", "prune", "-f"])
}

pub fn build_image_stream(path: String, tag: Option<String>) -> Result<Receiver<ProgressEvent>> {
    let mut command = Command::new("nerdctl");
    command.arg("build");

    if let Some(tag) = tag.as_deref() {
        command.args(["-t", tag]);
    }

    command.arg(path);

    spawn_docker_stream(
        command,
        "failed to start nerdctl build",
        "build completed successfully",
        "build",
    )
}

fn spawn_docker_stream(
    mut command: Command,
    start_context: &'static str,
    success_message: &'static str,
    action_name: &'static str,
) -> Result<Receiver<ProgressEvent>> {
    let (tx, rx) = mpsc::channel();
    let success_message = success_message.to_string();
    let action_name = action_name.to_string();

    thread::spawn(move || {
        let mut child = match command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                let _ = tx.send(ProgressEvent::Finished {
                    success: false,
                    message: format!("{start_context}: {err}"),
                });
                return;
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let mut readers = Vec::new();

        if let Some(stdout) = stdout {
            readers.push(spawn_reader(stdout, tx.clone()));
        }

        if let Some(stderr) = stderr {
            readers.push(spawn_reader(stderr, tx.clone()));
        }

        let wait_result = child.wait();

        for reader in readers {
            let _ = reader.join();
        }

        let event = match wait_result {
            Ok(status) if status.success() => ProgressEvent::Finished {
                success: true,
                message: success_message,
            },
            Ok(status) => ProgressEvent::Finished {
                success: false,
                message: format!("{action_name} exited with status {status}"),
            },
            Err(err) => ProgressEvent::Finished {
                success: false,
                message: format!("failed to wait for {action_name}: {err}"),
            },
        };

        let _ = tx.send(event);
    });

    Ok(rx)
}

fn spawn_reader<R>(reader: R, tx: Sender<ProgressEvent>) -> thread::JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 4096];
        // Accumulate raw bytes so a multi-byte UTF-8 character split across two
        // reads is decoded at a line boundary rather than per-chunk (which would
        // turn it into U+FFFD replacement characters).
        let mut pending: Vec<u8> = Vec::new();
        // Index of the first unprocessed byte in `pending`. Searching and emitting
        // lines advances this cursor; the processed prefix is dropped once per
        // read instead of once per line, keeping the whole loop O(n) rather than
        // O(n²) for output that uses many '\r' progress updates.
        let mut start = 0usize;

        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => pending.extend_from_slice(&buf[..n]),
                Err(_) => break,
            }

            // nerdctl push/build emit progress with '\r' updates, not just '\n',
            // so split on either to stream output live instead of buffering until the end.
            while let Some(rel) = pending[start..]
                .iter()
                .position(|&b| b == b'\n' || b == b'\r')
            {
                let pos = start + rel;
                let delim = pending[pos];

                // Borrow the line bytes directly; from_utf8_lossy returns a
                // borrowed Cow when the bytes are already valid UTF-8, so no
                // extra allocation is needed in the common case.
                let line = String::from_utf8_lossy(&pending[start..pos]).into_owned();
                start = pos + 1;

                // consume the '\n' of a '\r\n' pair so it isn't emitted as a blank line
                if delim == b'\r' && pending.get(start) == Some(&b'\n') {
                    start += 1;
                }

                if !line.trim().is_empty() {
                    let _ = tx.send(ProgressEvent::Line(line));
                }
            }

            // Drop the processed prefix once per read to bound `pending`'s growth.
            if start > 0 {
                pending.drain(..start);
                start = 0;
            }
        }

        if start < pending.len() {
            let line = String::from_utf8_lossy(&pending[start..]).into_owned();
            if !line.trim().is_empty() {
                let _ = tx.send(ProgressEvent::Line(line));
            }
        }
    })
}

fn run_docker(args: &[&str]) -> Result<String> {
    let output = Command::new("nerdctl")
        .args(args)
        .output()
        .context("failed to execute docker command")?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        if !stdout.is_empty() {
            Ok(stdout)
        } else if !stderr.is_empty() {
            Ok(stderr)
        } else {
            Ok("command completed successfully".to_string())
        }
    } else if !stderr.is_empty() {
        bail!(stderr)
    } else if !stdout.is_empty() {
        bail!(stdout)
    } else {
        bail!("docker exited with status {}", output.status)
    }
}

fn parse_container(line: &str) -> Option<Container> {
    let mut fields = line.split('\t');
    Some(Container {
        id: fields.next()?.trim().to_string(),
        image: fields.next()?.trim().to_string(),
        command: fields.next()?.trim().to_string(),
        status: fields.next()?.trim().to_string(),
        names: fields.next()?.trim().to_string(),
    })
}

fn parse_image(line: &str) -> Option<Image> {
    let mut fields = line.split('\t');
    Some(Image {
        repository: fields.next()?.trim().to_string(),
        tag: fields.next()?.trim().to_string(),
        id: fields.next()?.trim().to_string(),
        size: fields.next()?.trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::{ProgressEvent, parse_container, parse_image, spawn_reader};

    /// A reader that hands back data one fixed-size chunk per `read` call,
    /// letting tests simulate multi-byte characters and lines split across
    /// reads — exactly the case the byte-accumulating reader must handle.
    struct ChunkedReader {
        chunks: Vec<Vec<u8>>,
        index: usize,
    }

    impl Read for ChunkedReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.index >= self.chunks.len() {
                return Ok(0);
            }
            let chunk = &self.chunks[self.index];
            let n = chunk.len().min(buf.len());
            buf[..n].copy_from_slice(&chunk[..n]);
            self.index += 1;
            Ok(n)
        }
    }

    fn collect_lines(chunks: Vec<Vec<u8>>) -> Vec<String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let reader = ChunkedReader { chunks, index: 0 };
        let handle = spawn_reader(reader, tx);
        handle.join().unwrap();

        let mut lines = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let ProgressEvent::Line(line) = event {
                lines.push(line);
            }
        }
        lines
    }

    #[test]
    fn splits_on_carriage_return_and_newline() {
        let lines = collect_lines(vec![b"pushing layer\rpulling done\nfinished\r\n".to_vec()]);

        assert_eq!(lines, vec!["pushing layer", "pulling done", "finished"]);
    }

    #[test]
    fn drops_blank_lines() {
        let lines = collect_lines(vec![b"\n\r\n  \n".to_vec()]);

        assert!(lines.is_empty());
    }

    #[test]
    fn handles_multibyte_utf8_split_across_reads() {
        // "é" is 0xC3 0xA9; split it across two reads so a per-chunk decoder
        // would emit U+FFFD. The reader must accumulate bytes and decode at
        // the line boundary instead.
        let lines = collect_lines(vec![b"caf".to_vec(), vec![0xC3], vec![0xA9, b'\n']]);

        assert_eq!(lines, vec!["café"]);
    }

    #[test]
    fn parses_container_rows() {
        let container = parse_container("abc123\tnginx\t\"nginx -g\"\tUp 5 minutes\tweb").unwrap();

        assert_eq!(container.id, "abc123");
        assert_eq!(container.image, "nginx");
        assert_eq!(container.status, "Up 5 minutes");
        assert_eq!(container.names, "web");
    }

    #[test]
    fn parses_image_rows() {
        let image = parse_image("nginx\tlatest\tsha256abc\t187MB").unwrap();

        assert_eq!(image.repository, "nginx");
        assert_eq!(image.tag, "latest");
        assert_eq!(image.id, "sha256abc");
        assert_eq!(image.size, "187MB");
    }
}
