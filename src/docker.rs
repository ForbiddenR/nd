use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

use anyhow::{Context, Result, bail};

use crate::app::{Container, Image};

#[derive(Debug)]
pub enum BuildEvent {
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

pub fn remove_image(id: &str) -> Result<String> {
    run_docker(&["rmi", id])
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

pub fn build_image_stream(path: String, tag: Option<String>) -> Result<Receiver<BuildEvent>> {
    let (tx, rx) = mpsc::channel();
    let mut command = Command::new("nerdctl");
    command.arg("build");

    if let Some(tag) = tag.as_deref() {
        command.args(["-t", tag]);
    }

    let mut child = command
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start nerdctl build")?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

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
            Ok(status) if status.success() => BuildEvent::Finished {
                success: true,
                message: "build completed successfully".to_string(),
            },
            Ok(status) => BuildEvent::Finished {
                success: false,
                message: format!("build exited with status {status}"),
            },
            Err(err) => BuildEvent::Finished {
                success: false,
                message: format!("failed to wait for build: {err}"),
            },
        };

        let _ = tx.send(event);
    });

    Ok(rx)
}

fn spawn_reader<R>(reader: R, tx: Sender<BuildEvent>) -> thread::JoinHandle<()>
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        for line in BufReader::new(reader)
            .lines()
            .map_while(std::result::Result::ok)
        {
            let _ = tx.send(BuildEvent::Line(line));
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
