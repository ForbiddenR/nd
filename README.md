# nd

A terminal UI for managing [nerdctl] containers, images, and builds.

`nd` is a small [Ratatui] app that wraps common `nerdctl` operations - listing,
starting/stopping/removing containers and images, building and pushing images -
behind a keyboard-driven interface. Long-running operations (build, push,
refresh, prune) run in the background so the UI never blocks, and each build or
push becomes a task on the Tasks screen where it can be inspected or
cancelled.

## Requirements

- [nerdctl] on `PATH`
- A terminal supporting the alternate screen

## Installation

Install the latest prebuilt Linux binary to `/usr/local/bin`:

```sh
curl -fsSL https://raw.githubusercontent.com/ForbiddenR/nd/rs/install.sh | bash
```

The release binary is built against musl and statically linked, so it has no
glibc dependency and runs on any Linux distribution regardless of glibc
version. The script resolves the latest release tag, downloads the matching
asset for your architecture, and installs `nd` to `/usr/local/bin/nd`
(re-running with
`sudo` automatically if that directory isn't writable).

Or build from source:

```sh
cargo install --path .
```

## Configuration

`nd` reads `nd.toml` in the working directory at startup and on refresh. Each
`[[configs]]` entry can resolve a build tag dynamically from a remote version:

```toml
[[configs]]
# tag_template's "{version}" is replaced with the version fetched from version_url
tag_template = "cswxn/claude:v{version}"
version_url = "https://registry.npmjs.org/@anthropic-ai/claude-code/latest"

[[configs]]
# without version_url, tag is used as-is
tag = "example/other:latest"
```

Resolved tags are offered as a selectable list on the Build screen.

## Screens & keys

| Screen | Keys |
| --- | --- |
| Containers | `r` refresh · `tab` switch screen · `↑/↓` select · `s` start · `x` stop · `R` restart · `d` remove · `p` system prune |
| Images | `r` refresh · `tab` switch · `↑/↓` select · `d` remove · `p` prune images · `s` push |
| Build | `r` reload config · `tab` switch · type path · `enter` build · `backspace` delete · `ctrl-u` clear · `p` prune builder |
| Tasks | `tab` switch · `↑/↓` select task · `d` remove task (cancels if running) · `c` clear finished output · `esc` cancel running task |
| Logs | `r` refresh · `tab` switch · `c` clear logs |

Global: `1`-`5` jump to a screen · `tab`/`shift-tab` cycle · `q` quit (confirms
if a task is still running) · `ctrl-c` force quit (kills running task
processes).

## License

Copyright (c) ForbiddenR

This project is licensed under the MIT license ([LICENSE] or <http://opensource.org/licenses/MIT>)

[LICENSE]: ./LICENSE

[nerdctl]: https://github.com/containerd/nerdctl
[Ratatui]: https://ratatui.rs
