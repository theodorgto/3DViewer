import * as vscode from 'vscode';
import { spawn } from 'node:child_process';
import { promises as fs } from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { randomBytes } from 'node:crypto';

export const viewType = '3dviewer.model';
const rendered = new vscode.EventEmitter<{ name: string; vertices: number; triangles: number; visiblePixels: number; warnings: string[] }>();

export function activate(context: vscode.ExtensionContext) {
	context.subscriptions.push(vscode.window.registerCustomEditorProvider(viewType, new ModelEditor(context), {
		webviewOptions: { retainContextWhenHidden: true }, supportsMultipleEditorsPerDocument: false,
	}));
	context.subscriptions.push(vscode.commands.registerCommand('3dviewer.open', async (uri?: vscode.Uri) => {
		const selected = uri ?? (await vscode.window.showOpenDialog({ canSelectMany: false, filters: { '3D models': ['step', 'stp', 'ply', 'stl', 'obj', 'gltf', 'glb'] } }))?.[0];
		if (selected) { await vscode.commands.executeCommand('vscode.openWith', selected, viewType); }
	}));
	context.subscriptions.push(rendered);
	return { onDidRender: rendered.event };
}

class ModelEditor implements vscode.CustomReadonlyEditorProvider {
	constructor(private readonly context: vscode.ExtensionContext) {}
	openCustomDocument(uri: vscode.Uri): vscode.CustomDocument {
		if (uri.scheme !== 'file') { throw new Error('3DViewer currently supports local files only.'); }
		return { uri, dispose() {} };
	}
	resolveCustomEditor(document: vscode.CustomDocument, panel: vscode.WebviewPanel, token: vscode.CancellationToken) {
		panel.webview.options = { enableScripts: true, localResourceRoots: [vscode.Uri.joinPath(this.context.extensionUri, 'dist')] };
		panel.webview.html = this.html(panel.webview);
		let active: AbortController | undefined;
		let disposed = false;
		let generation = 0;
		const load = async () => {
			active?.abort();
			const controller = new AbortController();
			active = controller;
			const current = ++generation;
			await panel.webview.postMessage({ type: 'loading', name: path.basename(document.uri.fsPath) });
			try {
				if (!vscode.workspace.isTrusted) { throw new Error('Trust this workspace before running native model import.'); }
				const configured = vscode.workspace.getConfiguration('3dviewer').get<string>('executablePath');
				const executable = configured || vscode.Uri.joinPath(this.context.extensionUri, 'bin', 'model-viewer.exe').fsPath;
				const tolerance = vscode.workspace.getConfiguration('3dviewer').get<number>('stepTolerance', 0.001);
				const model = await importModel(executable, document.uri.fsPath, tolerance, controller.signal);
				if (!disposed && current === generation) { await panel.webview.postMessage({ type: 'model', model }); }
			} catch (error) {
				if (!disposed && current === generation) {
					await panel.webview.postMessage({ type: 'error', error: controller.signal.aborted ? 'Import cancelled' : String(error instanceof Error ? error.message : error) });
				}
			} finally { if (current === generation) { active = undefined; } }
		};
		const receive = panel.webview.onDidReceiveMessage(async message => {
			if (message?.type === 'rendered') { rendered.fire(message); }
			if (message?.type === 'ready' || message?.type === 'reload') { await load(); }
			if (message?.type === 'cancel') { active?.abort(); }
			if (message?.type === 'external' && vscode.workspace.isTrusted) {
				const configured = vscode.workspace.getConfiguration('3dviewer').get<string>('executablePath');
				const executable = configured || vscode.Uri.joinPath(this.context.extensionUri, 'bin', 'model-viewer.exe').fsPath;
				const child = spawn(executable, [document.uri.fsPath], { detached: true, stdio: 'ignore', windowsHide: false });
				child.once('error', error => { void vscode.window.showErrorMessage(error.message); }); child.unref();
			}
		});
		const cancellation = token.onCancellationRequested(() => active?.abort());
		panel.onDidDispose(() => { disposed = true; generation++; active?.abort(); receive.dispose(); cancellation.dispose(); });
	}
	private html(webview: vscode.Webview): string {
		const nonce = randomBytes(18).toString('base64');
		const script = webview.asWebviewUri(vscode.Uri.joinPath(this.context.extensionUri, 'dist', 'webview.js'));
		const style = webview.asWebviewUri(vscode.Uri.joinPath(this.context.extensionUri, 'dist', 'webview.css'));
		const logo = webview.asWebviewUri(vscode.Uri.joinPath(this.context.extensionUri, 'dist', 'logo.png'));
		return `<!doctype html><html><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${webview.cspSource} data:; style-src ${webview.cspSource}; script-src 'nonce-${nonce}';"><link rel="stylesheet" href="${style}"></head><body><div id="app" data-logo="${logo}"></div><script nonce="${nonce}" src="${script}"></script></body></html>`;
	}
}

export async function importModel(executable: string, filename: string, tolerance: number, signal: AbortSignal): Promise<unknown> {
	if (signal.aborted) { throw new Error('Import cancelled'); }
	const directory = await fs.mkdtemp(path.join(os.tmpdir(), '3dviewer-vscode-'));
	const output = path.join(directory, 'model.json');
	try {
		await new Promise<void>((resolve, reject) => {
			const child = spawn(executable, ['--export-model', filename, String(tolerance), output], { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
			let detail = ''; let failure: Error | undefined;
			child.stderr.on('data', chunk => { detail = (detail + String(chunk)).slice(-8192); });
			const cancel = () => { failure = new Error('Import cancelled'); child.kill(); };
			signal.addEventListener('abort', cancel, { once: true });
			if (signal.aborted) { cancel(); }
			const timeout = setTimeout(() => { failure = new Error('Import timed out after three minutes. Try a coarser STEP tolerance.'); child.kill(); }, 180000);
			const clean = () => { clearTimeout(timeout); signal.removeEventListener('abort', cancel); };
			child.once('error', error => { clean(); reject(new Error(`Cannot run the Rust importer: ${error.message}`)); });
			child.once('close', code => {
				clean();
				if (failure) { reject(failure); }
				else if (code !== 0) { reject(new Error(detail.split(/\r?\n/).find(line => line.startsWith('Import failed:')) || 'The native model importer failed.')); }
				else { resolve(); }
			});
		});
		return JSON.parse(await fs.readFile(output, 'utf8'));
	} finally { await fs.rm(directory, { recursive: true, force: true }); }
}
