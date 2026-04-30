export type DockerBuildArg = {
  name: string;
  value: string;
};

export type DockerBuildOptions = {
  contextDir: string;
  dockerfilePath: string;
  imageTag: string;
  buildArgs: DockerBuildArg[];
  onOutput: (text: string) => void;
};

export type DockerBuildRun = {
  promise: Promise<number>;
  cancel: () => void;
};

export function startDockerBuild(options: DockerBuildOptions): DockerBuildRun {
  let proc: Bun.Subprocess<"pipe", "pipe", "pipe">;

  const cmd = [
    "nerdctl",
    "build",
    "--file",
    options.dockerfilePath,
    "--tag",
    options.imageTag,
  ];

  for (const buildArg of options.buildArgs) {
    cmd.push("--build-arg", `${buildArg.name}=${buildArg.value}`);
  }

  cmd.push(options.contextDir);

  try {
    proc = Bun.spawn({
      cmd,
      stdout: "pipe",
      stderr: "pipe",
    });
  } catch (error) {
    return {
      promise: Promise.reject(error),
      cancel: () => {},
    };
  }

  const promise = Promise.all([
    readStream(proc.stdout ?? null, options.onOutput),
    readStream(proc.stderr ?? null, options.onOutput),
    proc.exited,
  ]).then(([, , exitCode]) => exitCode);

  return {
    promise,
    cancel: () => proc.kill(),
  };
}

async function readStream(
  stream: ReadableStream<Uint8Array> | null,
  onText: (text: string) => void,
): Promise<void> {
  if (!stream) {
    return;
  }

  const reader = stream.getReader();
  const decoder = new TextDecoder();

  while (true) {
    const { value, done } = await reader.read();

    if (done) {
      break;
    }

    onText(decoder.decode(value, { stream: true }));
  }

  const finalText = decoder.decode();

  if (finalText) {
    onText(finalText);
  }
}
