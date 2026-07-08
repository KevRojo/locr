import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';

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

    // TODO: replace with real locr import once the engine is wired.
    const text = await imageToTextPlaceholder(bytes);

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

async function imageToTextPlaceholder(bytes: Buffer): Promise<string> {
  console.log('OCR placeholder received', bytes.length, 'bytes');
  return `[placeholder OCR] ${bytes.length} bytes procesados desde ${path.basename('image')}. Reemplazar con import de locr.`;
}

export function deactivate() {}
