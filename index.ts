import React from "react";
import { render } from "ink";
import { App } from "./App.tsx";

render(React.createElement(App, { argv: Bun.argv.slice(2) }));