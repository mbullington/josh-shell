import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const configured = vscode.workspace
    .getConfiguration("josh")
    .get<string[]>("server.command", ["josh", "lsp"]);
  const [command, ...args] = configured;
  if (!command) {
    void vscode.window.showErrorMessage(
      "josh.server.command must name an executable, e.g. [\"josh\", \"lsp\"]",
    );
    return;
  }

  const serverOptions: ServerOptions = {
    command,
    args,
    transport: TransportKind.stdio,
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "josh" }],
  };
  client = new LanguageClient(
    "josh",
    "Josh Language Server",
    serverOptions,
    clientOptions,
  );
  context.subscriptions.push(
    new vscode.Disposable(() => {
      void client?.stop();
    }),
  );
  void client.start();
}

export async function deactivate(): Promise<void> {
  await client?.stop();
}
