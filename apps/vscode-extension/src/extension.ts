import * as vscode from 'vscode';
import { BridgeClient } from './BridgeClient';

let bridge: BridgeClient;

export function activate(context: vscode.ExtensionContext) {
  const port = vscode.workspace.getConfiguration('moses').get<number>('port', 43210);
  bridge = new BridgeClient(`ws://127.0.0.1:${port}`);
  bridge.connect();

  bridge.onMessage((msg: any) => {
    if (msg.type === 'PatchProposal') {
      vscode.window
        .showInformationMessage(
          `Moses: changes to ${(msg.files as string[]).join(', ')}`,
          'Accept', 'Reject'
        )
        .then(action => {
          if (action === 'Accept') {
            bridge.send({ type: 'AcceptPatch', diff: msg.diff });
          } else {
            bridge.send({ type: 'RejectPatch' });
          }
        });
    }
    if (msg.type === 'Error') {
      vscode.window.showErrorMessage(`Moses: ${msg.message}`);
    }
  });

  context.subscriptions.push(
    vscode.commands.registerCommand('moses.openPanel', () => {
      vscode.window.showInformationMessage('Moses: Open the Moses desktop app to chat.');
    }),

    vscode.commands.registerCommand('moses.askAboutSelection', () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) return;
      const text = editor.document.getText(editor.selection);
      const file = editor.document.fileName;
      bridge.send({
        type: 'Prompt',
        text: `Explain this code in ${file}:\n\`\`\`\n${text}\n\`\`\``,
        workspace: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? '',
        open_files: [file],
      });
      vscode.window.showInformationMessage('Moses: Sent to assistant');
    }),

    vscode.commands.registerCommand('moses.refactorSelection', () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) return;
      const text = editor.document.getText(editor.selection);
      const file = editor.document.fileName;
      bridge.send({
        type: 'Prompt',
        text: `Refactor this code in ${file}:\n\`\`\`\n${text}\n\`\`\``,
        workspace: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? '',
        open_files: [file],
      });
    })
  );

  // Status bar
  const bar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  bar.text = '$(robot) Moses';
  bar.command = 'moses.openPanel';
  bar.show();
  context.subscriptions.push(bar);
}

export function deactivate() {
  bridge?.disconnect();
}
