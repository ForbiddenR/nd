# nd

A Bun-powered terminal UI for building Docker images from folders that already contain a `Dockerfile`.

## Requirements

- Bun
- nerdctl with a running container runtime
- A target folder containing an existing `Dockerfile`

## Install

```bash
bun install
```

## Run

Review and build the current folder:

```bash
bun run start
```

Build another folder:

```bash
bun run start /path/to/project
```

Set an initial image tag:

```bash
bun run start /path/to/project --tag my-image:latest
```

The TUI shows the build context and Dockerfile path, discovers Dockerfile `ARG` declarations, and lets you edit the image tag and ARG values before running `nerdctl build`.

## Configuration

`nd` reads `nd.json` from the selected build context:

```json
{
  "tag": "my-image:latest"
}
```

Tag priority:

1. `--tag` / `-t`
2. `nd.json` `tag`
3. generated folder-name fallback

After a successful build, `nd` updates `nd.json` with the final validated tag from the TUI.

## TUI controls

- Up/Down or Tab: move between fields
- Enter or e: edit the selected tag or ARG field
- Space: enable or disable the selected ARG
- b: start the build from anywhere on the review screen
- q or Esc: quit on the review screen, or cancel while building

While editing a field:

- Type to insert text
- Left/Right: move the cursor
- Home/End or Ctrl+A/Ctrl+E: jump to start/end
- Backspace/Delete: remove text
- Ctrl+U: clear the field
- Enter: save
- Esc: cancel editing

## Build binary

```bash
bun run build
```

The compiled binary is written to `dist/nd`.
