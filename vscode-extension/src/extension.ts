import * as vscode from 'vscode';
import { spawn } from 'node:child_process';
import { promises as fs } from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { randomBytes } from 'node:crypto';
import { StringDecoder } from 'node:string_decoder';

export interface ImportProgress { fraction: number | null; stage: string; }

export function parseImportProgress(line: string): ImportProgress | undefined {
	const prefix = 'MODEL_PROGRESS: ';
	if (!line.startsWith(prefix)) { return; }
	try {
		const progress = JSON.parse(line.slice(prefix.length));
		if (!progress || typeof progress.stage !== 'string' || !progress.stage.length || progress.stage.length > 160) { return; }
		if (progress.fraction !== null && (typeof progress.fraction !== 'number' || !Number.isFinite(progress.fraction) || progress.fraction < 0 || progress.fraction > 1)) { return; }
		return { fraction: progress.fraction, stage: progress.stage };
	} catch { return; }
}

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
				const model = await importModel(executable, document.uri.fsPath, tolerance, controller.signal, progress => {
					if (!disposed && current === generation && !controller.signal.aborted) {
						void panel.webview.postMessage({ type: 'progress', ...progress });
					}
				});
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

export async function importModel(executable: string, filename: string, tolerance: number, signal: AbortSignal, onProgress?: (progress: ImportProgress) => void): Promise<unknown> {
	if (signal.aborted) { throw new Error('Import cancelled'); }
	const directory = await fs.mkdtemp(path.join(os.tmpdir(), '3dviewer-vscode-'));
	const output = path.join(directory, 'model.json');
	try {
		onProgress?.({ fraction: null, stage: 'Reading model' });
		await new Promise<void>((resolve, reject) => {
			const child = spawn(executable, ['--export-model', filename, String(tolerance), output], { windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'], env: { ...process.env, MODEL_VIEWER_IMPORT_PROGRESS: '1' } });
			let detail = ''; let failure: Error | undefined;
			const decoder = new StringDecoder('utf8');
			let pending = '';
			const reportLine = (line: string) => {
				const progress = parseImportProgress(line);
				if (progress && !failure && !signal.aborted) {
					onProgress?.({ ...progress, fraction: progress.fraction === null ? null : progress.fraction * 0.95 });
				}
			};
			child.stderr.on('data', chunk => {
				const text = decoder.write(chunk);
				detail = (detail + text).slice(-8192);
				const lines = (pending + text).split(/\r?\n/);
				pending = (lines.pop() || '').slice(-8192);
				for (const line of lines) { reportLine(line); }
			});
			const cancel = () => { failure = new Error('Import cancelled'); child.kill(); };
			signal.addEventListener('abort', cancel, { once: true });
			if (signal.aborted) { cancel(); }
			const timeout = setTimeout(() => { failure = new Error('Import timed out after three minutes. Try a coarser STEP tolerance.'); child.kill(); }, 180000);
			const clean = () => { clearTimeout(timeout); signal.removeEventListener('abort', cancel); };
			child.once('error', error => { clean(); reject(new Error(`Cannot run the Rust importer: ${error.message}`)); });
			child.once('close', code => {
				clean();
				const tail = decoder.end();
				detail = (detail + tail).slice(-8192);
				reportLine(pending + tail);
				if (failure) { reject(failure); }
				else if (code !== 0) { reject(new Error(detail.split(/\r?\n/).find(line => line.startsWith('Import failed:')) || 'The native model importer failed.')); }
				else { resolve(); }
			});
		});
		if (signal.aborted) { throw new Error('Import cancelled'); }
		onProgress?.({ fraction: 0.97, stage: 'Reading imported geometry' });
		const contents = await fs.readFile(output, 'utf8');
		if (signal.aborted) { throw new Error('Import cancelled'); }
		const model = JSON.parse(contents);
		onProgress?.({ fraction: 1, stage: 'Preparing view' });
		return model;
	} finally { await fs.rm(directory, { recursive: true, force: true }); }
}
