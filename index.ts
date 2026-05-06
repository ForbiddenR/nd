import React from "react";
import { render } from "ink";
import { App } from "./App.tsx";
import packageJson from "./package.json" with { type: "json" };

const argv = Bun.argv.slice(2);

if (argv.length === 1 && (argv[0] === "-v" || argv[0] === "--version")) {
  console.log(`${packageJson.name} ${packageJson.version}`);
} else {
  render(React.createElement(App, { argv }));
}