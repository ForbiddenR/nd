import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { basename, join, resolve } from "node:path";
import { readFileSync, writeFileSync } from "node:fs";
import { Box, Text, useApp, useInput } from "ink";
import { startDockerBuild, type DockerBuildArg, type DockerBuildRun } from "./dockerBuild.ts";
import packageJson from "./package.json" with { type: "json" };

const MAX_OUTPUT_LINES = 80;
const BUILD_ARG_NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;

type Phase =
  | "checking"
  | "invalid"
  | "missingDockerfile"
  | "review"
  | "building"
  | "success"
  | "failed"
  | "cancelled";

type ParsedArgs = {
  contextDir: string;
  dockerfilePath: string;
  configPath: string;
  error: string | null;
  config: Config;
};

type BuildArgDefinition = {
  name: string;
  defaultValue: string | null;
  occurrences: number;
};

type BuildArgValue = {
  name: string;
  defaultValue: string | null;
  value: string;
  enabled: boolean;
  occurrences: number;
};

type EditField =
  | { kind: "tag" }
  | { kind: "init" }
  | { kind: "arg"; index: number };

type EditState = {
  field: EditField;
  draft: string;
  cursor: number;
} | null;

type BuildConfig = {
  buildArgs: DockerBuildArg[];
};

type Config = Record<string, unknown> & {
  tag?: string;
  versionCheckUrl?: string;
};

type NpmMetadata = {
  latest: string;
};

type AppProps = {
  argv: string[];
};

export function App({ argv }: AppProps) {
  const { exit } = useApp();
  const inputIsActive = useMemo(() => supportsRawInput(), []);
  const parsed = useMemo(() => parseArgs(argv), [argv]);
  const [lastTag, setLastTag] = useState(parsed.config.tag ?? "");
  const [version, setVersion] = useState<string | null>(null);
  const imageTag = useMemo(() => `${lastTag}:v${version ?? "latest"}`, [lastTag, version]);
  const [phase, setPhase] = useState<Phase>(parsed.error ? "invalid" : "checking");
  const [buildArgs, setBuildArgs] = useState<BuildArgValue[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [editState, setEditState] = useState<EditState>(null);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [outputLines, setOutputLines] = useState<string[]>([]);
  const [exitCode, setExitCode] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(parsed.error);
  const [canBuild, setCanBuild] = useState(parsed.config.tag ? true : false);
  const [inputPrefix, setInputPrefix] = useState("");
  const buildRunRef = useRef<DockerBuildRun | null>(null);
  const buildConfigRef = useRef<BuildConfig | null>(null);
  const fetchedDefaultVersionRef = useRef<string | null>(null);
  const cancelledRef = useRef(false);


  const buildIndex = buildArgs.length + 1;

  const appendOutput = useCallback((text: string) => {
    setOutputLines((current) => {
      const lines = text.replaceAll("\r", "\n").split("\n");
      return [...current, ...lines].slice(-MAX_OUTPUT_LINES);
    });
  }, []);

  const startBuildFromReview = useCallback(() => {
    const tagError = validateImageTag(imageTag);

    if (tagError) {
      setValidationError(tagError);
      return;
    }

    const enabledBuildArgs = buildArgs
      .filter((arg) => arg.enabled)
      .map((arg) => ({ name: arg.name, value: arg.value }));
    const argNameError = enabledBuildArgs
      .map((arg) => validateBuildArgName(arg.name))
      .find((message): message is string => message !== null);

    if (argNameError) {
      setValidationError(argNameError);
      return;
    }

    buildConfigRef.current = {
      buildArgs: enabledBuildArgs,
    };

    setOutputLines([]);
    setExitCode(null);
    setError(null);
    setValidationError(null);
    setEditState(null);
    setPhase("building");
  }, [buildArgs, imageTag]);

  const moveSelection = useCallback(
    (direction: number) => {
      const fieldCount = buildIndex + 1;
      setValidationError(null);
      setSelectedIndex((current) => (current + direction + fieldCount) % fieldCount);
    },
    [buildIndex],
  );

  const startEditingSelected = useCallback(() => {
    setValidationError(null);

    if (!canBuild) {
      setEditState({ field: { kind: "init" }, draft: inputPrefix, cursor: inputPrefix.length });
      return;
    }

    if (selectedIndex === 0) {
      setEditState({ field: { kind: "tag" }, draft: parsed.config.tag ?? "", cursor: parsed.config.tag?.length ?? 0 });
      return;
    }

    const argIndex = selectedIndex - 1;
    const arg = buildArgs[argIndex];

    if (!arg) {
      return;
    }

    setEditState({ field: { kind: "arg", index: argIndex }, draft: arg.value, cursor: arg.value.length });
  }, [buildArgs, imageTag, selectedIndex]);

  // const saveEdit = useCallback(() => {
  //   if (!editState) {
  //     return;
  //   }

  //   if (editState.field.kind === "tag") {
  //     setImageTag(editState.draft);
  //     setEditState(null);
  //     setValidationError(null);
  //     return;
  //   }

  //   const fieldIndex = editState.field.index;
  //   const draft = editState.draft;

  //   setBuildArgs((current) => current.map((arg, index) => {
  //     if (index !== fieldIndex) {
  //       return arg;
  //     }

  //     return { ...arg, value: draft, enabled: true };
  //   }));
  //   setEditState(null);
  //   setValidationError(null);
  // }, [editState]);

  const toggleSelectedArg = useCallback(() => {
    const argIndex = selectedIndex - 1;

    if (argIndex < 0 || argIndex >= buildArgs.length) {
      return;
    }

    setValidationError(null);
    setBuildArgs((current) => current.map((arg, index) => {
      if (index !== argIndex) {
        return arg;
      }

      return { ...arg, enabled: !arg.enabled };
    }));
  }, [buildArgs.length, selectedIndex]);

  useEffect(() => {
    let cancelled = false;
    const versionCheckUrl = parsed.config.versionCheckUrl;
    if (!versionCheckUrl) {
      return;
    }

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 2000);

    checkLatestVersion(versionCheckUrl, controller.signal)
      .then((result) => {
        if (!cancelled && result) {
          fetchedDefaultVersionRef.current = result;
          setVersion(result);
        }
      })
      .catch(() => { });

    return () => {
      cancelled = true;
      clearTimeout(timer);
      controller.abort();
    };
  }, [parsed.configPath, parsed.contextDir]);

  useEffect(() => {
    if (parsed.error) {
      setError(parsed.error);
      setPhase("invalid");
      return;
    }

    let cancelled = false;

    setError(null);
    setValidationError(null);
    setEditState(null);
    setPhase("checking");

    Bun.file(parsed.dockerfilePath)
      .exists()
      .then(async (exists) => {
        if (cancelled) {
          return;
        }

        if (!exists) {
          setPhase("missingDockerfile");
          return;
        }

        const dockerfileText = await Bun.file(parsed.dockerfilePath).text();
        const definitions = parseDockerfileArgs(dockerfileText);

        if (cancelled) {
          return;
        }
        setBuildArgs(definitions.map((definition) => ({
          name: definition.name,
          defaultValue: definition.defaultValue,
          value: definition.defaultValue ?? "",
          enabled: definition.defaultValue !== null,
          occurrences: definition.occurrences,
        })));
        setSelectedIndex(definitions.length + 1);
        setPhase("review");
      })
      .catch((reason: unknown) => {
        if (cancelled) {
          return;
        }

        setError(reason instanceof Error ? reason.message : String(reason));
        setPhase("invalid");
      });

    return () => {
      cancelled = true;
    };
  }, [parsed.contextDir, parsed.dockerfilePath, parsed.error]);

  useEffect(() => {
    if (lastTag !== "") {
      Bun.file(parsed.configPath)
        .write(JSON.stringify({ ...parsed.config, tag: lastTag }, null, 2))
        .catch((e) => { process.stderr.write(`Failed to update .nd.json with new tag: ${e instanceof Error ? e.message : String(e)}\n`); });
    }
  }, [lastTag])

  useEffect(() => {
    if (phase !== "building" || buildRunRef.current) {
      return;
    }

    const buildConfig = buildConfigRef.current;

    if (!buildConfig) {
      process.exitCode = 1;
      setError("Missing build configuration.");
      setPhase("failed");
      return;
    }

    cancelledRef.current = false;

    const run = startDockerBuild({
      contextDir: parsed.contextDir,
      dockerfilePath: parsed.dockerfilePath,
      imageTag: imageTag,
      buildArgs: buildConfig.buildArgs,
      onOutput: appendOutput,
    });

    buildRunRef.current = run;

    run.promise
      .then((code) => {
        buildRunRef.current = null;
        setExitCode(code);

        if (cancelledRef.current) {
          process.exitCode = 130;
          setPhase("cancelled");
          return;
        }

        if (code !== 0) {
          process.exitCode = code;
          setPhase("failed");
          return;
        }

        try {
          writeFileSync(parsed.configPath, JSON.stringify(parsed.config))
        } catch (reason: unknown) {
          process.exitCode = 1;
          setError(reason instanceof Error
            ? `Build succeeded, but failed to update .nd.json: ${reason.message}`
            : `Build succeeded, but failed to update .nd.json: ${String(reason)}`);
          setPhase("failed");
          return;
        }

        setPhase("success");
      })
      .catch((reason: unknown) => {
        buildRunRef.current = null;

        if (cancelledRef.current) {
          setPhase("cancelled");
          return;
        }

        process.exitCode = 1;
        setError(reason instanceof Error ? reason.message : String(reason));
        setPhase("failed");
      });
  }, [appendOutput, parsed.configPath, parsed.contextDir, parsed.dockerfilePath, phase]);

  useEffect(() => {
    if (inputIsActive || phase === "checking") {
      return;
    }

    if (phase === "review") {
      process.exitCode = 1;
      setError("Interactive terminal input is required to start the build.");
      setPhase("invalid");
      return;
    }

    if (phase === "invalid" || phase === "missingDockerfile" || phase === "failed") {
      process.exitCode = exitCode && exitCode > 0 ? exitCode : 1;
    }

    const timer = setTimeout(exit, 50);

    return () => clearTimeout(timer);
  }, [exit, exitCode, inputIsActive, phase]);

  useInput(
    (input, key) => {
      if (phase === "building") {
        if (input === "q" || key.escape) {
          cancelledRef.current = true;
          buildRunRef.current?.cancel();
        }

        return;
      }

      if (phase === "review") {
        if (!canBuild) {
          if (key.return && inputPrefix !== "") {
            setLastTag(inputPrefix);
            setCanBuild(true);
            return;
          }
          setEditState((current) => editInput(current, input, key));
        }

        if (editState) {
          if (key.escape) {
            setEditState(null);
            return;
          }

          if (key.return) {
            // saveEdit();
            return;
          }

          setEditState((current) => editInput(current, input, key));
          return;
        }

        if (key.upArrow || (key.tab && key.shift)) {
          moveSelection(-1);
          return;
        }

        if (key.downArrow || key.tab) {
          moveSelection(1);
          return;
        }

        if (input === " ") {
          toggleSelectedArg();
          return;
        }

        if (input === "e") {
          startEditingSelected();
          return;
        }

        if (input === "b") {
          startBuildFromReview();
          return;
        }

        if (key.return) {
          if (selectedIndex === buildIndex) {
            startBuildFromReview();
            return;
          }

          startEditingSelected();
          return;
        }

        if (input === "q" || key.escape) {
          exit();
        }

        return;
      }

      if (input === "q" || key.escape) {
        exit();
      }
    },
    { isActive: inputIsActive },
  );

  return (
    <Box flexDirection="column" gap={1}>
      <Text bold>nd docker build</Text>
      <BuildSummary contextDir={parsed.contextDir} dockerfilePath={parsed.dockerfilePath} />
      {phase === "review" ? (
        <ReviewEditor
          imageTag={imageTag}
          buildArgs={buildArgs}
          selectedIndex={selectedIndex}
          editState={editState}
          validationError={validationError}
          canBuild={canBuild}
        />
      ) : (
        <PhaseView
          phase={phase}
          outputLines={outputLines}
          exitCode={exitCode}
          error={error}
        />
      )}
    </Box>
  );
}

function BuildSummary({ contextDir, dockerfilePath }: { contextDir: string; dockerfilePath: string }) {
  return (
    <Box flexDirection="column">
      <Text>Context:    {contextDir}</Text>
      <Text>Dockerfile: {dockerfilePath}</Text>
    </Box>
  );
}

function ReviewEditor({
  imageTag,
  buildArgs,
  selectedIndex,
  editState,
  validationError,
  canBuild,
}: {
  imageTag: string;
  buildArgs: BuildArgValue[];
  selectedIndex: number;
  editState: EditState;
  validationError: string | null;
  canBuild: boolean;
}) {
  const buildIndex = buildArgs.length + 1;

  return (
    <Box flexDirection="column">
      <Text bold>Build options</Text>
      {
        canBuild ? (
          <>
            <Text color={selectedIndex === buildIndex ? "cyan" : undefined}>{selectedIndex === buildIndex ? "› " : "  "}[Start build]</Text>
            <FieldRow selected={selectedIndex === 0} label="Tag" value={imageTag} editing={editState?.field.kind === "tag"} draft={editState?.field.kind === "tag" ? editState.draft : null} cursor={editState?.field.kind === "tag" ? editState.cursor : null} />
            {buildArgs.length === 0 ? (
              <Text dimColor>No Dockerfile ARG values found.</Text>
            ) : (
              buildArgs.map((arg, index) => (
                <FieldRow
                  key={arg.name}
                  selected={selectedIndex === index + 1}
                  label={`ARG ${arg.name}`}
                  value={formatBuildArgValue(arg)}
                  editing={editState?.field.kind === "arg" && editState.field.index === index}
                  draft={editState?.field.kind === "arg" && editState.field.index === index ? editState.draft : null}
                  cursor={editState?.field.kind === "arg" && editState.field.index === index ? editState.cursor : null}
                  suffix={arg.occurrences > 1 ? `declared ${arg.occurrences} times` : null}
                />
              ))
            )}
            {validationError ? <Text color="red">{validationError}</Text> : null}
            {editState ? (
              <Box flexDirection="column">
                <Text dimColor>Type to edit. Enter saves. Esc cancels.</Text>
                <Text dimColor>Left/Right move cursor. Home/End or Ctrl+A/Ctrl+E jump. Ctrl+U clears.</Text>
              </Box>
            ) : (
              <Box flexDirection="column">
                <Text dimColor>Up/Down or Tab moves. Enter/e edits. Space toggles ARG. b builds.</Text>
                <Text dimColor>Press q or Esc to quit.</Text>
              </Box>
            )}
          </>
        ) : (<FieldRow selected={false} label={"Fixed Prefix"} value={""} editing={true} draft={editState?.field.kind === "init" ? editState.draft : null} cursor={editState?.field.kind === "init" ? editState.cursor : null} />)
      }
    </Box >
  )
}

function FieldRow({
  selected,
  label,
  value,
  editing,
  draft,
  cursor,
  suffix = "",
}: {
  selected: boolean;
  label: string;
  value: string;
  editing: boolean;
  draft: string | null;
  cursor: number | null;
  suffix?: string | null;
}) {
  return (
    <Text color={selected ? "cyan" : undefined}>
      {selected ? "› " : "  "}{label}: {editing && draft !== null && cursor !== null ? renderDraft(draft, cursor) : value}{suffix}
    </Text>
  );
}

function renderDraft(draft: string, cursor: number): React.ReactNode {
  const before = draft.slice(0, cursor);
  const current = draft[cursor] ?? " ";
  const after = draft.slice(cursor + 1);

  return (
    <>
      {before}
      <Text inverse>{current}</Text>
      {after}
    </>
  );
}

function formatBuildArgValue(arg: BuildArgValue): string {
  if (!arg.enabled) {
    return "<unset>";
  }

  if (arg.value === "") {
    return "\"\"";
  }

  return arg.value;
}

function PhaseView({
  phase,
  outputLines,
  exitCode,
  error,
}: {
  phase: Phase;
  outputLines: string[];
  exitCode: number | null;
  error: string | null;
}) {
  if (phase === "checking") {
    return <Text>Checking for Dockerfile...</Text>;
  }

  if (phase === "invalid") {
    return (
      <Box flexDirection="column">
        <Text color="red">{error}</Text>
        <Text>Press q to exit.</Text>
      </Box>
    );
  }

  if (phase === "missingDockerfile") {
    return (
      <Box flexDirection="column">
        <Text color="red">No Dockerfile found at the selected path.</Text>
        <Text>This tool builds existing Dockerfile-based projects and does not create Dockerfiles.</Text>
        <Text>Press q to exit.</Text>
      </Box>
    );
  }

  if (phase === "building") {
    return (
      <Box flexDirection="column">
        <Text color="cyan">Building image. Press q or Esc to cancel.</Text>
        <Output lines={outputLines} />
      </Box>
    );
  }

  if (phase === "success") {
    return (
      <Box flexDirection="column">
        <Text color="green">Build completed successfully.</Text>
        <Output lines={outputLines} />
        <Text>Press q to exit.</Text>
      </Box>
    );
  }

  if (phase === "cancelled") {
    return (
      <Box flexDirection="column">
        <Text color="yellow">Build cancelled.</Text>
        <Output lines={outputLines} />
        <Text>Press q to exit.</Text>
      </Box>
    );
  }

  return (
    <Box flexDirection="column">
      <Text color="red">Build failed{exitCode === null ? "." : ` with exit code ${exitCode}.`}</Text>
      {error ? <Text color="red">{error}</Text> : null}
      <Output lines={outputLines} />
      <Text>Press q to exit.</Text>
    </Box>
  );
}

function Output({ lines }: { lines: string[] }) {
  const visibleLines = lines.filter((line, index) => line.length > 0 || index === lines.length - 1);

  if (visibleLines.length === 0) {
    return <Text dimColor>No output yet.</Text>;
  }

  return (
    <Box flexDirection="column">
      {visibleLines.map((line, index) => (
        <Text key={`${index}-${line}`}>{line || " "}</Text>
      ))}
    </Box>
  );
}

function editInput(current: EditState, input: string, key: Parameters<Parameters<typeof useInput>[0]>[1]): EditState {
  if (!current) {
    return current;
  }

  if (key.leftArrow) {
    return { ...current, cursor: Math.max(0, current.cursor - 1) };
  }

  if (key.rightArrow) {
    return { ...current, cursor: Math.min(current.draft.length, current.cursor + 1) };
  }

  if (key.home) {
    return { ...current, cursor: 0 };
  }

  if (key.end) {
    return { ...current, cursor: current.draft.length };
  }

  if (key.ctrl && input === "a") {
    return { ...current, cursor: 0 };
  }

  if (key.ctrl && input === "e") {
    return { ...current, cursor: current.draft.length };
  }

  if (key.ctrl && input === "u") {
    return { ...current, draft: "", cursor: 0 };
  }

  if (key.backspace) {
    if (current.cursor === 0) {
      return current;
    }

    return {
      ...current,
      draft: current.draft.slice(0, current.cursor - 1) + current.draft.slice(current.cursor),
      cursor: current.cursor - 1,
    };
  }

  if (key.delete) {
    if (current.cursor === current.draft.length) {
      return current;
    }

    return {
      ...current,
      draft: current.draft.slice(0, current.cursor) + current.draft.slice(current.cursor + 1),
    };
  }

  if (input && !key.ctrl && !key.meta && !key.tab) {
    return {
      ...current,
      draft: current.draft.slice(0, current.cursor) + input + current.draft.slice(current.cursor),
      cursor: current.cursor + input.length,
    };
  }

  return current;
}

function supportsRawInput(): boolean {
  const stdin = process.stdin as typeof process.stdin & {
    isTTY?: boolean;
    setRawMode?: (enabled: boolean) => void;
  };

  return stdin.isTTY === true && typeof stdin.setRawMode === "function";
}

function parseArgs(argv: string[]): ParsedArgs {
  let contextArg: string | null = null;
  let imageTag: string | null = null;
  let config: Config = {};

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (!arg) {
      continue;
    }

    if (arg === "--tag" || arg === "-t") {
      const value = argv[index + 1];

      if (!value) {
        return invalidArgs("Missing value for --tag.", config);
      }

      imageTag = value;
      index += 1;
      continue;
    }

    if (arg.startsWith("--tag=")) {
      imageTag = arg.slice("--tag=".length);
      continue;
    }

    if (arg.startsWith("-")) {
      return invalidArgs(`Unknown option: ${arg}`, config);
    }

    if (contextArg) {
      return invalidArgs(`Unexpected argument: ${arg}`, config);
    }

    contextArg = arg;
  }

  const contextDir = resolve(contextArg ?? process.cwd());
  const configPath = join(contextDir, ".nd.json");

  config = readNdConfig(configPath) ?? {};

  if (imageTag) {
    config.tag = imageTag;
  }

  return {
    contextDir,
    dockerfilePath: join(contextDir, "Dockerfile"),
    configPath,
    config,
    error: null,
  };
}

function invalidArgs(error: string, config: Config): ParsedArgs {
  const contextDir = process.cwd();
  const configPath = join(contextDir, ".nd.json");

  return {
    contextDir,
    dockerfilePath: join(contextDir, "Dockerfile"),
    configPath,
    config,
    error,
  };
}

function readNdConfig(configPath: string): Config | null {
  try {
    const text = readFileSync(configPath, "utf-8");
    const json = JSON.parse(text);
    return json ? json : null;
  } catch {
    return null;
  }
}

// function readNdConfig(configPath: string): NdConfig {
//   try {
//     const parsed = JSON.parse(readFileSync(configPath, "utf-8"));

//     if (!isRecord(parsed)) {
//       return {};
//     }

//     return parsed;
//   } catch {
//     return {};
//   }
// }

// function readTag(configPath: string): string | null {
//   const { tag } = readNdConfig(configPath);

//   return typeof tag === "string" ? tag : null;
// }

// function readVersionCheckUrl(configPath: string): string | null {
//   const { versionCheckUrl } = readNdConfig(configPath);
//   return versionCheckUrl ? versionCheckUrl : null;
// }

// function writeNdConfig(configPath: string, tag: string): void {
//   const next = {
//     ...readNdConfig(configPath),
//     tag,
//   };

//   writeFileSync(configPath, `${JSON.stringify(next, null, 2)}\n`, "utf-8");
// }

// function isRecord(value: unknown): value is NdConfig {
//   return typeof value === "object" && value !== null && !Array.isArray(value);
// }

async function checkLatestVersion(
  versionCheckUrl: string,
  signal: AbortSignal,
): Promise<string | null> {
  const response = await fetch(versionCheckUrl, { signal });

  if (!response.ok) {
    return null;
  }

  return extractNpmLatestVersion(await response.json() as NpmMetadata);
}

function extractNpmLatestVersion(metadata: NpmMetadata): string | null {
  const latest = metadata?.latest;
  return latest && typeof latest === "string" ? latest : null;
}

function parseDockerfileArgs(text: string): BuildArgDefinition[] {
  const definitions: BuildArgDefinition[] = [];
  const indexByName = new Map<string, number>();

  for (const rawLine of text.split(/\r?\n/)) {
    const line = stripDockerfileComment(rawLine).trim();
    const match = /^ARG\s+([^=\s]+)(?:\s*=\s*(.*))?$/i.exec(line);

    if (!match) {
      continue;
    }

    const name = match[1];

    if (!name || !BUILD_ARG_NAME_PATTERN.test(name)) {
      continue;
    }

    const defaultValue = match[2] === undefined ? null : unquoteDockerArgDefault(match[2].trim());
    const existingIndex = indexByName.get(name);

    if (existingIndex !== undefined) {
      const existing = definitions[existingIndex];

      if (!existing) {
        continue;
      }

      definitions[existingIndex] = {
        ...existing,
        defaultValue: existing.defaultValue ?? defaultValue,
        occurrences: existing.occurrences + 1,
      };
      continue;
    }

    indexByName.set(name, definitions.length);
    definitions.push({ name, defaultValue, occurrences: 1 });
  }

  return definitions;
}

function stripDockerfileComment(line: string): string {
  let quote: "\"" | "'" | null = null;

  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];

    if ((char === "\"" || char === "'") && line[index - 1] !== "\\") {
      quote = quote === char ? null : quote ?? char;
      continue;
    }

    if (char === "#" && quote === null) {
      return line.slice(0, index);
    }
  }

  return line;
}

function unquoteDockerArgDefault(value: string): string {
  if (value.length >= 2) {
    const first = value[0];
    const last = value[value.length - 1];

    if ((first === "\"" && last === "\"") || (first === "'" && last === "'")) {
      return value.slice(1, -1);
    }
  }

  return value;
}

function validateImageTag(imageTag: string): string | null {
  if (!imageTag) {
    return "Image tag cannot be empty.";
  }

  if (/\s/.test(imageTag)) {
    return "Image tag cannot contain whitespace.";
  }

  if (imageTag.startsWith("-")) {
    return "Image tag cannot start with '-'.";
  }

  return null;
}

function validateBuildArgName(name: string): string | null {
  if (!BUILD_ARG_NAME_PATTERN.test(name)) {
    return `Invalid build arg name: ${name}`;
  }

  return null;
}
