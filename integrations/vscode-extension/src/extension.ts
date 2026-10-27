import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import { imageToText } from 'locr';

export function activate(context: vscode.ExtensionContext) {
  const disposable = vscode.commands.registerCommand('locr.imageToText', async (uri?: vscode.Uri) => {
    let filePath: string | undefined;

    if (uri && uri.fsPath) {
      filePath = uri.fsPath;
    } else {
      const uris = await vscode.window.showOpenDialog({
        canSelectFiles: true,
        canSelectFolders: false,
        canSelectMany: false,
        filters: {
          Images: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp', 'tiff']
        }
      });
      if (!uris || uris.length === 0) { return; }
      filePath = uris[0].fsPath;
    }

    if (!filePath) { return; }

    const bytes = fs.readFileSync(filePath);

    let text: string;
    try {
      // Runs the locr WASM engine inside the extension host: nothing is uploaded.
      text = await vscode.window.withProgress(
        { location: vscode.ProgressLocation.Notification, title: `locr: reading ${path.basename(filePath)}` },
        () => imageToText(bytes),
      );
    } catch (err) {
      vscode.window.showErrorMessage(`locr: ${err instanceof Error ? err.message : String(err)}`);
      return;
    }
    if (!text) {
      vscode.window.showInformationMessage(`locr: no text found in ${path.basename(filePath)}`);
      return;
    }

    const editor = vscode.window.activeTextEditor;
    if (editor) {
      editor.edit((editBuilder) => {
        editBuilder.insert(editor.selection.active, text);
      });
    } else {
      vscode.window.showInformationMessage(text);
    }
  });

  context.subscriptions.push(disposable);
}

export function deactivate() {}
