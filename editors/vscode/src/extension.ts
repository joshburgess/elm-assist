import * as path from "path";
import { workspace, ExtensionContext, window } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  Executable,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
  const config = workspace.getConfiguration("elm-lint");

  if (!config.get<boolean>("enable", true)) {
    return;
  }

  const serverPath = findServerPath(config.get<string>("serverPath", ""));

  if (!serverPath) {
    window.showWarningMessage(
      "elm-assist-lsp binary not found. Install it or set elm-lint.serverPath in settings."
    );
    return;
  }

  const serverExecutable: Executable = {
    command: serverPath,
  };

  const serverOptions: ServerOptions = {
    run: serverExecutable,
    debug: serverExecutable,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "elm" }],
    synchronize: {
      fileEvents: [
        workspace.createFileSystemWatcher("**/*.elm"),
        workspace.createFileSystemWatcher("**/elm-assist.toml"),
      ],
    },
  };

  client = new LanguageClient(
    "elm-lint",
    "Elm Lint",
    serverOptions,
    clientOptions
  );

  await client.start();
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}

function findServerPath(configPath: string): string | undefined {
  // 1. Explicit config setting.
  if (configPath) {
    return configPath;
  }

  // 2. Project-local node_modules.
  const workspaceFolders = workspace.workspaceFolders;
  if (workspaceFolders) {
    for (const folder of workspaceFolders) {
      const localPath = path.join(
        folder.uri.fsPath,
        "node_modules",
        ".bin",
        "elm-assist-lsp"
      );
      try {
        require("fs").accessSync(localPath, require("fs").constants.X_OK);
        return localPath;
      } catch {
        // Not found here, try next.
      }
    }
  }

  // 3. Fall back to PATH.
  return "elm-assist-lsp";
}
