import readline from "node:readline";

import type { RuntimeHostMessage } from "@cellsymphony/device-contracts";

import { createCoreRunner, type CoreRunnerOptions } from "./index";

export function startCoreRunnerStdio(options: CoreRunnerOptions = {}): void {
  const runner = createCoreRunner(options);
  const rl = readline.createInterface({ input: process.stdin });
  rl.on("line", (line) => {
    const trimmed = line.trim();
    if (!trimmed) return;
    const message = JSON.parse(trimmed) as RuntimeHostMessage;
    for (const output of runner.dispatch(message)) {
      process.stdout.write(`${JSON.stringify(output)}\n`);
    }
  });
}
