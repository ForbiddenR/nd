# nd

A terminal UI for managing [nerdctl] containers, images, and builds.

`nd` is a small [Ratatui] app that wraps common `nerdctl` operations - listing,
starting/stopping/removing containers and images, building and pushing images -
behind a keyboard-driven interface. Long-running operations (build, push,
refresh, prune) run in the background so the UI never blocks.

## Requirements

- [nerdctl] on `PATH`
- A terminal supporting the alternate screen

## Installation

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
| Build Status | `tab` switch · `c` clear output · `esc` cancel running build |
| Push Status | `tab` switch · `c` clear output · `esc` cancel running push |
| Logs | `r` refresh · `tab` switch · `c` clear logs |

Global: `1`-`6` jump to a screen · `tab`/`shift-tab` cycle · `q` quit (confirms
if a build or push is still running) · `ctrl-c` force quit (kills running
build/push processes).

## License

Copyright (c) ForbiddenR

This project is licensed under the MIT license ([LICENSE] or <http://opensource.org/licenses/MIT>)

[LICENSE]: ./LICENSE

[nerdctl]: https://github.com/containerd/nerdctl
[Ratatui]: https://ratatui.rs
