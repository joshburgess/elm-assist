"use strict";

const path = require("path");
const { spawnSync } = require("child_process");

module.exports = function run(name) {
  const ext = process.platform === "win32" ? ".exe" : "";
  const bin = path.join(__dirname, "..", "vendor", `${name}${ext}`);
  const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });

  if (result.error) {
    if (result.error.code === "ENOENT") {
      console.error(`${name} binary not found. Try running "npm install" again.`);
      process.exit(1);
    }
    throw result.error;
  }

  process.exit(result.status ?? 1);
};
