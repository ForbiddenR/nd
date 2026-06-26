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
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(start_context)?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let success_message = success_message.to_string();
    let action_name = action_name.to_string();

    thread::spawn(move || {
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
        let mut pending = String::new();

        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => pending.push_str(&String::from_utf8_lossy(&buf[..n])),
                Err(_) => break,
            }

            // nerdctl push/build emit progress with '\r' updates, not just '\n',
            // so split on either to stream output live instead of buffering until the end.
            while let Some(pos) = pending.find(|c: char| c == '\n' || c == '\r') {
                let delim = pending[pos..].chars().next().unwrap();
                let delim_len = delim.len_utf8();
                let line: String = pending.drain(..pos).collect();
                pending.drain(..delim_len);

                // consume the '\n' of a '\r\n' pair so it isn't emitted as a blank line
                if delim == '\r' && pending.starts_with('\n') {
                    pending.drain(..1);
                }

                if !line.trim().is_empty() {
                    let _ = tx.send(ProgressEvent::Line(line));
                }
            }
        }

        if !pending.trim().is_empty() {
            let _ = tx.send(ProgressEvent::Line(pending));
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
    use super::{parse_container, parse_image};

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
