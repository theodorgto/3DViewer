import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { createElement, Maximize2, RotateCcw, RefreshCw, ExternalLink, X } from 'lucide';

declare function acquireVsCodeApi(): { postMessage(message: unknown): void };
interface ModelData {
	name: string;
	vertices: { position: number[]; normal: number[]; color: number[] }[];
	indices: number[];
	dimensions: number[];
	warnings: string[];
}
const host = acquireVsCodeApi();
const app = document.getElementById('app')!;
app.innerHTML = `<header><img class="brand-logo" src="${app.dataset.logo || 'dist/logo.png'}" alt="3DViewer" width="28" height="28"><strong id="name">3DViewer</strong><div class="tools">
<button id="fit" title="Fit model" aria-label="Fit model"></button><button id="reset" title="Reset view" aria-label="Reset view"></button>
<button id="reload" title="Reload model" aria-label="Reload model"></button><button id="external" title="Open in desktop viewer" aria-label="Open in desktop viewer"></button>
<button id="cancel" title="Cancel import" aria-label="Cancel import" hidden></button></div>
<div class="modes" role="group" aria-label="Display mode"><button data-mode="solid" aria-pressed="true">Solid</button><button data-mode="wireframe" aria-pressed="false">Wireframe</button><button data-mode="points" aria-pressed="false">Points</button></div>
<label><input id="grid" type="checkbox" checked>Grid</label><label><input id="orthographic" type="checkbox">Orthographic</label>
<label><input id="colors" type="checkbox" checked>File colors</label><label><input id="flat" type="checkbox" checked>Flat</label>
<label>Up<select id="up" aria-label="Up axis"><option>Z</option><option>Y</option></select></label>
<label id="point-control" hidden>Size<input id="point-size" type="range" min="1" max="12" value="3" aria-label="Point size"></label></header>
<div id="warning" role="alert" hidden></div><div id="error" role="alert" hidden></div>
<main id="viewport"><div id="empty">Loading model</div></main>
<footer><progress id="activity" max="1" aria-label="Model loading progress"></progress><span id="status" role="status">Loading</span><span id="statistics"></span></footer>`;
for (const [identifier, icon] of [['fit', Maximize2], ['reset', RotateCcw], ['reload', RefreshCw], ['external', ExternalLink], ['cancel', X]] as const) {
	document.getElementById(identifier)!.append(createElement(icon));
}
const viewport = document.getElementById('viewport')!;
const error = document.getElementById('error')!;
const warning = document.getElementById('warning')!;
const status = document.getElementById('status')!;
const activity = document.getElementById('activity') as HTMLProgressElement;
const empty = document.getElementById('empty')!;
const checked = (identifier: string) => (document.getElementById(identifier) as HTMLInputElement).checked;
let renderer: THREE.WebGLRenderer;
try { renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true }); }
catch {
	error.textContent = 'WebGL is unavailable. Enable GPU acceleration in VS Code or open the desktop viewer.';
	error.hidden = false;
	activity.hidden = true; empty.hidden = true; status.textContent = 'WebGL unavailable';
	for (const control of document.querySelectorAll<HTMLButtonElement | HTMLInputElement | HTMLSelectElement>('header button, header input, header select')) {
		control.disabled = control.id !== 'external' && control.id !== 'reload';
	}
	document.getElementById('external')!.addEventListener('click', () => host.postMessage({ type: 'external' }));
	document.getElementById('reload')!.addEventListener('click', () => location.reload());
	throw new Error('WebGL unavailable');
}
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
renderer.domElement.setAttribute('aria-label', '3D model viewport');
viewport.append(renderer.domElement);
const scene = new THREE.Scene();
scene.background = new THREE.Color(getComputedStyle(viewport).backgroundColor);
scene.add(new THREE.HemisphereLight(0xffffff, 0x424840, 1.7));
const light = new THREE.DirectionalLight(0xffffff, 2.1);
light.position.set(4, -3, 6); scene.add(light);
const grid = new THREE.GridHelper(5, 20, 0x687773, 0x39413e);
scene.add(grid);
let camera: THREE.PerspectiveCamera | THREE.OrthographicCamera = new THREE.PerspectiveCamera(45, 1, 0.001, 1000);
camera.up.set(0, 0, 1); camera.position.set(3, -4, 3);
let controls: OrbitControls<THREE.PerspectiveCamera | THREE.OrthographicCamera> = new OrbitControls(camera, renderer.domElement);
controls.mouseButtons.MIDDLE = THREE.MOUSE.PAN;
controls.addEventListener('change', render);
let model: ModelData | undefined;
let geometry: THREE.BufferGeometry | undefined;
let object: THREE.Mesh | THREE.Points | undefined;
let mode = 'solid';
let importing = true;
let started = Date.now();
let loadingStage = 'Reading model';
let loadGeneration = 0;
function render() { renderer.render(scene, camera); }

function updateLoadingStatus() {
	status.textContent = `${loadingStage} (${Math.floor((Date.now() - started) / 1000)}s)`;
	activity.setAttribute('aria-valuetext', loadingStage);
	empty.textContent = loadingStage;
	viewport.setAttribute('aria-busy', 'true');
}

function finishImport() {
	importing = false; activity.hidden = true;
	document.getElementById('cancel')!.hidden = true;
	viewport.setAttribute('aria-busy', 'false');
}

function resize() {
	const width = Math.max(viewport.clientWidth, 1); const height = Math.max(viewport.clientHeight, 1);
	renderer.setSize(width, height, false);
	if (camera instanceof THREE.PerspectiveCamera) { camera.aspect = width / height; }
	else {
		const halfHeight = (camera.top - camera.bottom) * 0.5;
		camera.left = -halfHeight * width / height; camera.right = halfHeight * width / height;
	}
	camera.updateProjectionMatrix(); render();
}

function fit() {
	const radius = geometry?.boundingSphere?.radius || 1.5;
	const aspect = Math.max(viewport.clientWidth / Math.max(viewport.clientHeight, 1), 0.05);
	const distance = radius / Math.sin(Math.PI / 8) * 1.15 / Math.min(aspect, 1);
	const direction = camera.position.clone().sub(controls.target).normalize();
	controls.target.set(0, 0, 0); camera.position.copy(direction.multiplyScalar(distance));
	if (camera instanceof THREE.OrthographicCamera) {
		const height = radius * 1.15 / Math.min(aspect, 1);
		camera.top = height; camera.bottom = -height; camera.left = -height * aspect; camera.right = height * aspect;
	}
	camera.zoom = 1; camera.updateProjectionMatrix(); controls.update(); render();
}

function configureGrid() {
	const zUp = (document.getElementById('up') as HTMLSelectElement).value === 'Z';
	grid.rotation.x = zUp ? Math.PI / 2 : 0;
	grid.position.set(0, zUp ? 0 : -1.05, zUp ? -1.05 : 0);
	grid.visible = checked('grid');
	camera.up.set(0, zUp ? 0 : 1, zUp ? 1 : 0);
	controls.update(); render();
}

function resetView() {
	const zUp = (document.getElementById('up') as HTMLSelectElement).value === 'Z';
	camera.position.set(3, zUp ? -4 : 3, zUp ? 3 : 4);
	configureGrid(); fit();
}

function rebuildObject() {
	if (!geometry) { return; }
	if (object) {
		scene.remove(object); (object.material as THREE.Material).dispose();
		if (object instanceof THREE.Points) { object.geometry.dispose(); }
	}
	if (mode === 'points') {
		const points = geometry.clone(); points.setIndex(null);
		object = new THREE.Points(points, new THREE.PointsMaterial({
			vertexColors: checked('colors'), color: checked('colors') ? 0xffffff : 0x7bb5a2,
			size: Number((document.getElementById('point-size') as HTMLInputElement).value), sizeAttenuation: false,
		}));
	} else {
		object = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({
			vertexColors: checked('colors'), color: checked('colors') ? 0xffffff : 0x7bb5a2,
			wireframe: mode === 'wireframe', flatShading: checked('flat'), side: THREE.DoubleSide, roughness: 0.65, metalness: 0.08,
		}));
	}
	scene.add(object);
	for (const button of document.querySelectorAll<HTMLButtonElement>('[data-mode]')) { button.setAttribute('aria-pressed', String(button.dataset.mode === mode)); }
	document.getElementById('point-control')!.hidden = mode !== 'points'; render();
}

function showModel(data: ModelData) {
	model = data;
	if (geometry) {
		if (object) { scene.remove(object); (object.material as THREE.Material).dispose(); if (object instanceof THREE.Points) { object.geometry.dispose(); } }
		geometry.dispose(); object = undefined;
	}
	geometry = new THREE.BufferGeometry();
	geometry.setAttribute('position', new THREE.Float32BufferAttribute(data.vertices.flatMap(vertex => vertex.position), 3));
	geometry.setAttribute('normal', new THREE.Float32BufferAttribute(data.vertices.flatMap(vertex => vertex.normal), 3));
	geometry.setAttribute('color', new THREE.Float32BufferAttribute(data.vertices.flatMap(vertex => vertex.color), 3));
	geometry.setIndex(data.indices); geometry.computeBoundingSphere();
	mode = data.indices.length ? 'solid' : 'points';
	(document.getElementById('up') as HTMLSelectElement).value = /\.gl(tf|b)$/i.test(data.name) ? 'Y' : 'Z';
	document.getElementById('name')!.textContent = data.name;
	document.getElementById('statistics')!.textContent = `${data.vertices.length.toLocaleString()} vertices | ${(data.indices.length / 3).toLocaleString()} triangles | ${data.dimensions.map(value => value.toFixed(3)).join(' x ')}`;
	warning.textContent = data.warnings.join('\n'); warning.hidden = !data.warnings.length;
	error.hidden = true; empty.hidden = true;
	status.textContent = data.warnings.length ? 'INCOMPLETE MODEL' : 'Ready';
	rebuildObject(); resetView();
	const gl = renderer.getContext(); const pixels = new Uint8Array(gl.drawingBufferWidth * gl.drawingBufferHeight * 4);
	gl.readPixels(0, 0, gl.drawingBufferWidth, gl.drawingBufferHeight, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
	let visiblePixels = 0;
	for (let offset = 0; offset < pixels.length; offset += 4) {
		if (Math.abs(pixels[offset] - pixels[0]) + Math.abs(pixels[offset + 1] - pixels[1]) + Math.abs(pixels[offset + 2] - pixels[2]) > 40) { visiblePixels++; }
	}
	host.postMessage({ type: 'rendered', name: data.name, vertices: data.vertices.length, triangles: data.indices.length / 3, visiblePixels, warnings: data.warnings });
}

window.addEventListener('message', event => {
	const message = event.data;
	if (message?.type === 'loading') {
		loadGeneration++; importing = true; started = Date.now(); loadingStage = 'Reading model';
		activity.hidden = false; activity.removeAttribute('value'); document.getElementById('cancel')!.hidden = false;
		document.getElementById('name')!.textContent = message.name; error.hidden = true;
		empty.hidden = !!model; updateLoadingStatus();
	} else if (message?.type === 'progress' && importing && typeof message.stage === 'string') {
		loadingStage = message.stage;
		if (typeof message.fraction === 'number' && Number.isFinite(message.fraction)) {
			activity.value = Math.max(activity.hasAttribute('value') ? activity.value : 0, Math.min(1, Math.max(0, message.fraction)));
		} else { activity.removeAttribute('value'); }
		updateLoadingStatus();
	} else if (message?.type === 'model') {
		const current = ++loadGeneration;
		loadingStage = 'Preparing view'; activity.value = 1; updateLoadingStatus();
		setTimeout(() => {
			if (current !== loadGeneration) { return; }
			try { showModel(message.model); }
			catch (failure) { error.textContent = String(failure); error.hidden = false; empty.hidden = true; status.textContent = 'Import failed'; }
			finally { finishImport(); }
		}, 0);
	} else if (message?.type === 'error') {
		loadGeneration++; finishImport();
		error.textContent = message.error; error.hidden = false; empty.hidden = true; status.textContent = message.error === 'Import cancelled' ? 'Import cancelled' : 'Import failed';
	}
});
setInterval(() => { if (importing) { updateLoadingStatus(); } }, 1000);
document.getElementById('fit')!.addEventListener('click', fit);
document.getElementById('reset')!.addEventListener('click', resetView);
for (const identifier of ['reload', 'cancel', 'external']) { document.getElementById(identifier)!.addEventListener('click', () => host.postMessage({ type: identifier })); }
for (const button of document.querySelectorAll<HTMLButtonElement>('[data-mode]')) { button.addEventListener('click', () => { mode = button.dataset.mode!; rebuildObject(); }); }
for (const identifier of ['colors', 'flat', 'point-size']) { document.getElementById(identifier)!.addEventListener('input', rebuildObject); }
document.getElementById('grid')!.addEventListener('change', configureGrid);
document.getElementById('up')!.addEventListener('change', resetView);
document.getElementById('orthographic')!.addEventListener('change', () => {
	const position = camera.position.clone(); const up = camera.up.clone(); const target = controls.target.clone(); controls.dispose();
	camera = checked('orthographic') ? new THREE.OrthographicCamera(-2, 2, 2, -2, 0.001, 1000) : new THREE.PerspectiveCamera(45, 1, 0.001, 1000);
	camera.position.copy(position); camera.up.copy(up); controls = new OrbitControls(camera, renderer.domElement);
	controls.target.copy(target); controls.mouseButtons.MIDDLE = THREE.MOUSE.PAN; controls.addEventListener('change', render); resize(); fit();
});
window.addEventListener('keydown', event => { if (event.key.toLowerCase() === 'f' && !(event.target instanceof HTMLInputElement)) { fit(); } });
new ResizeObserver(resize).observe(viewport);
resize(); configureGrid();
host.postMessage({ type: 'ready' });