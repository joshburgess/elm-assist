"use strict";

const https = require("https");
const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const os = require("os");

const VERSION = require("./package.json").version;
const REPO = "joshburgess/elm-assist";

const TARGETS = {
  "darwin-x64": "x86_64-apple-darwin",
  "darwin-arm64": "aarch64-apple-darwin",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
};

function getTarget() {
  const key = `${os.platform()}-${os.arch()}`;
  const target = TARGETS[key];
  if (!target) {
    console.error(`Unsupported platform: ${key}`);
    console.error(`Supported: ${Object.keys(TARGETS).join(", ")}`);
    process.exit(1);
  }
  return target;
}

function fetch(url) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (res) => {
        if (res.statusCode === 301 || res.statusCode === 302) {
          return fetch(res.headers.location).then(resolve, reject);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode}: ${url}`));
        }
        const chunks = [];
        res.on("data", (chunk) => chunks.push(chunk));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      })
      .on("error", reject);
  });
}

async function main() {
  const target = getTarget();
  const isWindows = target.includes("windows");
  const ext = isWindows ? "zip" : "tar.gz";
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/elm-assist-v${VERSION}-${target}.${ext}`;
  const vendorDir = path.join(__dirname, "vendor");

  fs.mkdirSync(vendorDir, { recursive: true });

  console.log(`Downloading elm-assist v${VERSION} for ${target}...`);
  const data = await fetch(url);

  const archive = path.join(vendorDir, `elm-assist.${ext}`);
  fs.writeFileSync(archive, data);

  if (isWindows) {
    execSync(
      `powershell -command "Expand-Archive -Force '${archive}' '${vendorDir}'"`,
      { stdio: "inherit" }
    );
  } else {
    execSync(`tar xzf "${archive}" -C "${vendorDir}"`, { stdio: "inherit" });
    // Ensure binaries are executable
    const bins = ["elm-lint", "elm-unused", "elm-deps", "elm-refactor", "elm-search", "elm-assist-lsp"];
    for (const bin of bins) {
      const p = path.join(vendorDir, bin);
      if (fs.existsSync(p)) fs.chmodSync(p, 0o755);
    }
  }

  fs.unlinkSync(archive);
  console.log("elm-assist installed successfully.");
}

main().catch((err) => {
  console.error(`Failed to install elm-assist: ${err.message}`);
  process.exit(1);
});
