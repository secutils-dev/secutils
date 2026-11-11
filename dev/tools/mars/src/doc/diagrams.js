// Interactive maps and 3D models in `figure.su-diagram` elements.
// Shared by the live viewer and the standalone export runtimes. No app/storage imports.
//
// The viewer parses a `geojson`, `topojson`, `stl`, or `su-mesh` fence and writes its data into the
// figure. The data is a JSON `<script>` payload, or `fig.__suPayload` for a large model in the live
// preview. This module only draws that payload and wires the controls.
// - The payload is document content. Every node is built with DOM calls and `textContent`.
//   Feature colors must match a hex pattern, and the only markup strings are fixed icons.
// - A map requests its basemap tiles from tile.openstreetmap.org. A model makes no request.
// - A small model and every map draw to SVG, so the serialized figure prints without script.
//   A large model (a payload with `positions`) draws to a WebGL canvas and keeps a PNG snapshot
//   for print.

// Creates a WebGL renderer for a triangle mesh on `canvas` and returns its draw function
// `(yaw, pitch, zoom, panX, panY, display)`. Angles are radians, and pan is in units of the
// 800 by 400 view. `display` is 'Solid', 'Wireframe', or 'Surface angle'.
// `mesh` holds `positions` (base64 Float32 values, nine for each triangle), `min` and `max`
// bounds, optional `colors`, and optional typed arrays in `__buffers`.
// Throws an Error when WebGL is unavailable or a shader fails to compile or link.
export function createStlGpu(canvas, mesh) {
    // `preserveDrawingBuffer` keeps the last frame readable for the PNG snapshot (`toDataURL`).
    const gl = canvas.getContext('webgl', { antialias: true, preserveDrawingBuffer: true });
    if (!gl) throw new Error('WebGL is required to display this model.');
    const decode = (encoded) => {
        const binary = atob(encoded),
            bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
        return new Float32Array(bytes.buffer);
    };
    const positions = mesh.__buffers ? mesh.__buffers.positions.slice() : decode(mesh.positions);
    // Center the model and scale its longest side to 1, so the shader needs no model bounds.
    const center = mesh.min.map((value, axis) => (value + mesh.max[axis]) / 2);
    const span = Math.max(...mesh.max.map((value, axis) => value - mesh.min[axis]), 0.000001);
    for (let i = 0; i < positions.length; i++) positions[i] = (positions[i] - center[i % 3]) / span;
    const shader = (type, source) => {
        const result = gl.createShader(type);
        gl.shaderSource(result, source);
        gl.compileShader(result);
        if (!gl.getShaderParameter(result, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(result));
        return result;
    };
    const program = gl.createProgram();
    gl.attachShader(
        program,
        shader(
            gl.VERTEX_SHADER,
            `attribute vec3 position; attribute vec3 vertexColor; uniform vec4 view; uniform vec2 pan; varying vec3 point; varying vec3 surfaceColor;
        void main() { float cy=cos(view.x), sy=sin(view.x), cp=cos(view.y), sp=sin(view.y);
        vec3 p=vec3(position.x*cy+position.z*sy,position.y,-position.x*sy+position.z*cy);
        p=vec3(p.x,p.y*cp-p.z*sp,p.y*sp+p.z*cp); point=p; surfaceColor=vertexColor;
        gl_Position=vec4(p.x*view.z+pan.x,p.y*view.w+pan.y,-p.z*0.5,1.0); }`,
        ),
    );
    // Screen-space derivatives give a face normal without a normal buffer. Without the extension
    // every face gets the same light value.
    const derivatives = gl.getExtension('OES_standard_derivatives');
    gl.attachShader(
        program,
        shader(
            gl.FRAGMENT_SHADER,
            `${derivatives ? '#extension GL_OES_standard_derivatives : enable\n' : ''}precision mediump float; varying vec3 point; varying vec3 surfaceColor; uniform float mode; uniform float colored;
        void main(){ float light=${derivatives ? 'abs(normalize(cross(dFdx(point),dFdy(point))).z)' : '0.7'};
        vec3 color=mix(vec3(0.12,0.42,0.72),vec3(0.42,0.72,0.94),light);
        if(colored>0.5) color=surfaceColor*(0.65+0.35*light);
        if(mode>0.5) color=mix(vec3(0.3,0.2,0.85),vec3(0.9,0.65,0.1),light);
        gl_FragColor=vec4(color,1.0); }`,
        ),
    );
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program));
    gl.useProgram(program);
    const upload = (values) => {
        const buffer = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
        gl.bufferData(gl.ARRAY_BUFFER, values, gl.STATIC_DRAW);
        return buffer;
    };
    const solid = upload(positions);
    const colorLocation = gl.getAttribLocation(program, 'vertexColor');
    const colored = gl.getUniformLocation(program, 'colored');
    let colorBuffer;
    if (mesh.colors) colorBuffer = upload(mesh.__buffers?.colors ?? decode(mesh.colors));
    let wire;
    const location = gl.getAttribLocation(program, 'position');
    gl.enableVertexAttribArray(location);
    const view = gl.getUniformLocation(program, 'view'),
        pan = gl.getUniformLocation(program, 'pan'),
        mode = gl.getUniformLocation(program, 'mode');
    gl.enable(gl.DEPTH_TEST);
    return (yaw, pitch, zoom, panX, panY, display) => {
        // Backing store: the CSS size times the device pixel ratio (2 at most), 1,600 pixels wide
        // at most.
        const width = Math.min(1600, Math.round((canvas.clientWidth || 800) * Math.min(devicePixelRatio || 1, 2)));
        const height = Math.round(width / ((canvas.clientWidth || 800) / (canvas.clientHeight || 400)));
        if (canvas.width !== width || canvas.height !== height) {
            canvas.width = width;
            canvas.height = height;
        }
        gl.viewport(0, 0, width, height);
        gl.clearColor(0.933, 0.953, 0.973, 1);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
        // Build the edge buffer on the first wireframe frame: three line segments for each triangle.
        if (display === 'Wireframe' && !wire) {
            const edges = new Float32Array(positions.length * 2);
            for (let i = 0, j = 0; i < positions.length; i += 9)
                for (const vertex of [0, 1, 1, 2, 2, 0])
                    for (let axis = 0; axis < 3; axis++) edges[j++] = positions[i + vertex * 3 + axis];
            wire = upload(edges);
        }
        gl.bindBuffer(gl.ARRAY_BUFFER, display === 'Wireframe' ? wire : solid);
        gl.vertexAttribPointer(location, 3, gl.FLOAT, false, 0, 0);
        if (colorBuffer && display !== 'Wireframe') {
            gl.enableVertexAttribArray(colorLocation);
            gl.bindBuffer(gl.ARRAY_BUFFER, colorBuffer);
            gl.vertexAttribPointer(colorLocation, 3, gl.FLOAT, false, 0, 0);
        } else {
            gl.disableVertexAttribArray(colorLocation);
            gl.vertexAttrib3f(colorLocation, 0.15, 0.45, 0.75);
        }
        gl.uniform1f(colored, colorBuffer ? 1 : 0);
        gl.uniform4f(view, yaw, pitch, 0.75 * zoom, 1.5 * zoom);
        gl.uniform2f(pan, panX / 400, -panY / 200);
        gl.uniform1f(mode, display === 'Surface angle' ? 1 : 0);
        gl.drawArrays(
            display === 'Wireframe' ? gl.LINES : gl.TRIANGLES,
            0,
            (positions.length / 3) * (display === 'Wireframe' ? 2 : 1),
        );
    };
}

// Draws each `figure.su-diagram` under `root` (or `root` itself) and adds its controls. A figure
// is set up once. `root` can be detached: the viewer draws a figure before it enters the page.
//
// Side effects on a figure: `__suGetView()` returns the current view, and `dataset.currentView`
// follows the view of a model. `__suDispose()` releases the WebGL context and listeners of a large
// model. "Save current view to source" dispatches a bubbling `su-save-model-view` event with the
// view as `detail`. The control exists only when the viewer set `__suSourceRange`.
//
// Throws an Error for GeoJSON that is malformed or has no coordinates. The figures after the
// failed one stay untouched. A large model without WebGL shows the error text in its figure.
export function initDiagrams(root) {
    const figures = root.matches?.('figure.su-diagram') ? [root] : root.querySelectorAll('figure.su-diagram');
    const svgNS = 'http://www.w3.org/2000/svg';
    const makeSvg = (name, attrs) => {
        const el = document.createElementNS(svgNS, name);
        for (const [key, value] of Object.entries(attrs)) el.setAttribute(key, value);
        return el;
    };
    for (const fig of figures) {
        if (fig.__suDiagramReady) continue;
        fig.__suDiagramReady = true;
        const { data, source } =
            fig.__suPayload ?? JSON.parse(fig.querySelector('script[type="application/json"]').textContent);
        const colored = fig.dataset.kind === 'su-mesh';
        const stl = fig.dataset.kind === 'stl' || colored;
        const large = stl && !!data.positions;
        for (const el of fig.querySelectorAll(
            ':scope > svg, :scope > canvas, :scope > figcaption, :scope > .su-diagram-controls',
        ))
            el.remove();
        const svg = large ? document.createElement('canvas') : makeSvg('svg', { viewBox: '0 0 800 400' });
        svg.classList.add('su-diagram-view');
        svg.tabIndex = 0;
        svg.setAttribute('role', 'img');
        svg.setAttribute(
            'aria-label',
            stl
                ? `Interactive ${colored ? '3MF' : 'STL'} model. Arrow keys rotate. Shift and arrow keys pan. Plus and minus zoom.`
                : 'Interactive map. Arrow keys pan. Plus and minus zoom.',
        );
        let gpu;
        try {
            gpu = large ? createStlGpu(svg, data) : null;
        } catch (error) {
            svg.getContext('webgl')?.getExtension('WEBGL_lose_context')?.loseContext();
            const message = document.createElement('p');
            message.textContent = error.message;
            fig.appendChild(message);
            continue;
        }
        const caption = document.createElement('figcaption');
        if (!stl) {
            const attribution = document.createElement('a');
            attribution.href = 'https://www.openstreetmap.org/copyright';
            attribution.target = '_blank';
            attribution.rel = 'noopener';
            attribution.textContent = '© OpenStreetMap contributors';
            caption.textContent = 'Drag to pan; use + / − to zoom. ';
            caption.appendChild(attribution);
        }
        const controls = document.createElement('div');
        controls.className = 'su-diagram-controls';
        controls.setAttribute('role', 'group');
        controls.setAttribute('aria-label', stl ? 'Model controls' : 'Map controls');
        const group = (name) => {
            const el = document.createElement('div');
            el.className = `su-diagram-${name}`;
            controls.appendChild(el);
            return el;
        };
        const actions = group('actions'),
            navigation = group('navigation'),
            modesGroup = group('modes'),
            rotation = group('rotation');
        // Web Mercator in the unit square. Latitude is clamped to the limit of the tile grid.
        const mercator = (point) => {
            const latitude = (Math.max(-85.05112878, Math.min(85.05112878, point[1])) * Math.PI) / 180;
            return [(point[0] + 180) / 360, (1 - Math.log(Math.tan(Math.PI / 4 + latitude / 2)) / Math.PI) / 2];
        };
        const initial = {
            yaw: -0.6,
            pitch: 0.5,
            zoom: 1,
            panX: 0,
            panY: 0,
            mode: 'Solid',
            ...(stl && fig.dataset.view ? JSON.parse(fig.dataset.view) : {}),
        };
        let { zoom, panX, panY, yaw, pitch, mode } = initial;
        const currentView = () => ({ yaw, pitch, zoom, panX, panY, mode });
        fig.__suGetView = currentView;
        const features = [],
            allPoints = [];
        // Flattens a GeoJSON value into `features` and validates every coordinate on the way.
        const collectGeometry = (geometry, properties = {}) => {
            if (!geometry) return;
            if (geometry.type === 'FeatureCollection') {
                if (!Array.isArray(geometry.features)) throw new Error('Expected a GeoJSON features array.');
                for (const feature of geometry.features) collectGeometry(feature);
                return;
            }
            if (geometry.type === 'Feature') {
                collectGeometry(geometry.geometry, geometry.properties ?? {});
                return;
            }
            if (geometry.type === 'GeometryCollection') {
                for (const child of geometry.geometries) collectGeometry(child, properties);
                return;
            }
            const depths = { Point: 0, MultiPoint: 1, LineString: 1, MultiLineString: 2, Polygon: 2, MultiPolygon: 3 };
            if (!(geometry.type in depths)) throw new Error('Unsupported GeoJSON geometry type.');
            const visit = (coords, depth) => {
                if (!Array.isArray(coords)) throw new Error('Expected GeoJSON coordinates.');
                if (depth) {
                    for (const child of coords) visit(child, depth - 1);
                    return;
                }
                if (
                    coords.length < 2 ||
                    !Number.isFinite(coords[0]) ||
                    !Number.isFinite(coords[1]) ||
                    Math.abs(coords[0]) > 180 ||
                    Math.abs(coords[1]) > 90
                )
                    throw new Error('Expected longitude from −180 to 180 and latitude from −90 to 90.');
                allPoints.push(mercator(coords));
            };
            visit(geometry.coordinates, depths[geometry.type]);
            features.push({ ...geometry, properties });
        };
        if (large) allPoints.push(data.min, data.max);
        else if (stl) {
            for (const triangle of data) allPoints.push(...triangle);
        } else collectGeometry(data);
        if (!allPoints.length) throw new Error('The map has no coordinates to display.');
        const min = [Infinity, Infinity, Infinity],
            max = [-Infinity, -Infinity, -Infinity];
        for (const point of allPoints)
            for (let i = 0; i < (stl ? 3 : 2); i++) {
                min[i] = Math.min(min[i], point[i]);
                max[i] = Math.max(max[i], point[i]);
            }
        const center = min.map((n, i) => (n + max[i]) / 2);
        const span = Math.max(max[0] - min[0], max[1] - min[1], stl ? max[2] - min[2] : 0, stl ? 0.000001 : 0.0003);
        const baseScale = 300 / span;
        // Maps a model or map point to the 800 by 400 view. A model rotates by yaw, then by pitch.
        const project = (point) => {
            let x = point[0] - center[0],
                y = point[1] - center[1],
                z = stl ? point[2] - center[2] : 0;
            if (stl) {
                [x, z] = [x * Math.cos(yaw) + z * Math.sin(yaw), -x * Math.sin(yaw) + z * Math.cos(yaw)];
                [y, z] = [y * Math.cos(pitch) - z * Math.sin(pitch), y * Math.sin(pitch) + z * Math.cos(pitch)];
                y = -y;
            }
            return [400 + panX + x * baseScale * zoom, 200 + panY + y * baseScale * zoom, z * baseScale * zoom];
        };
        const draw = () => {
            if (stl) fig.dataset.currentView = JSON.stringify(currentView());
            if (gpu) {
                gpu(yaw, pitch, zoom, panX, panY, mode);
                return;
            }
            svg.replaceChildren();
            if (stl) {
                // Painter's algorithm: draw the triangles in the order of their projected depth.
                const triangles = data.map((triangle) => triangle.map(project));
                triangles.sort((a, b) => a.reduce((sum, p) => sum + p[2], 0) - b.reduce((sum, p) => sum + p[2], 0));
                for (const triangle of triangles) {
                    const [a, b, c] = triangle;
                    const u = b.map((n, i) => n - a[i]),
                        v = c.map((n, i) => n - a[i]);
                    const normal = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                    const light = Math.abs(normal[2]) / (Math.hypot(...normal) || 1);
                    svg.appendChild(
                        makeSvg('polygon', {
                            points: triangle.map((p) => `${p[0]},${p[1]}`).join(' '),
                            fill:
                                mode === 'Wireframe'
                                    ? 'none'
                                    : `hsl(${mode === 'Surface angle' ? 240 - light * 200 : 210} 65% ${45 + light * 20}%)`,
                            stroke: mode === 'Wireframe' ? '#2674bf' : 'none',
                            'stroke-width': '0.6',
                        }),
                    );
                }
            } else {
                // The whole world spans `worldSize` view units. Use the deepest tile level, 19 at
                // most, whose tiles still draw at 256 units or more.
                const worldSize = baseScale * zoom;
                const level = Math.max(0, Math.min(19, Math.floor(Math.log2(worldSize / 256))));
                const count = 2 ** level,
                    tileSize = worldSize / count;
                const origin = project([0, 0]);
                const startX = Math.floor(-origin[0] / tileSize),
                    startY = Math.floor(-origin[1] / tileSize);
                for (let x = startX; x <= startX + Math.ceil(800 / tileSize); x++) {
                    for (
                        let y = Math.max(0, startY);
                        y <= Math.min(count - 1, startY + Math.ceil(400 / tileSize));
                        y++
                    ) {
                        svg.appendChild(
                            makeSvg('image', {
                                x: origin[0] + x * tileSize,
                                y: origin[1] + y * tileSize,
                                width: tileSize + 0.5,
                                height: tileSize + 0.5,
                                href: `https://tile.openstreetmap.org/${level}/${((x % count) + count) % count}/${y}.png`,
                            }),
                        );
                    }
                }
                const line = (coords) =>
                    coords
                        .map((p, i) => {
                            const q = project(mercator(p));
                            return `${i ? 'L' : 'M'}${q[0]},${q[1]}`;
                        })
                        .join('');
                for (const feature of features) {
                    const { type, coordinates, properties } = feature;
                    const group = makeSvg('g', {}),
                        title = makeSvg('title', {});
                    title.textContent = JSON.stringify(properties);
                    group.appendChild(title);
                    // A style property from the document becomes an attribute only as a hex color.
                    const color = (value, fallback) =>
                        typeof value === 'string' && /^#[\da-f]{3}(?:[\da-f]{3})?$/i.test(value) ? value : fallback;
                    if (type === 'Point' || type === 'MultiPoint') {
                        for (const point of type === 'Point' ? [coordinates] : coordinates) {
                            const p = project(mercator(point));
                            group.appendChild(
                                makeSvg('circle', {
                                    cx: p[0],
                                    cy: p[1],
                                    r: 5,
                                    fill: color(properties['marker-color'], '#2674bf'),
                                    stroke: '#fff',
                                }),
                            );
                        }
                    } else {
                        const polygon = type.includes('Polygon');
                        const parts =
                            type === 'LineString'
                                ? [coordinates]
                                : type === 'MultiPolygon'
                                  ? coordinates.flat()
                                  : coordinates;
                        group.appendChild(
                            makeSvg('path', {
                                d: parts.map((part) => `${line(part)}${polygon ? 'Z' : ''}`).join(''),
                                fill: polygon ? color(properties.fill, '#8957e5') : 'none',
                                'fill-opacity': '0.3',
                                'fill-rule': 'evenodd',
                                stroke: color(properties.stroke, '#2674bf'),
                                'stroke-width': '2',
                            }),
                        );
                    }
                    svg.appendChild(group);
                }
            }
        };
        const button = (label, action, parent = navigation, icon) => {
            const btn = document.createElement('button');
            btn.type = 'button';
            btn.textContent = label;
            btn.setAttribute('aria-label', label);
            btn.title = label;
            if (icon) btn.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true">${icon}</svg>`;
            btn.addEventListener('click', async () => {
                await action();
                draw();
            });
            parent.appendChild(btn);
            return btn;
        };
        const scale = (factor) => {
            zoom = Math.max(0.1, Math.min(20, zoom * factor));
        };
        button('Zoom in', () => scale(1.25), navigation, '<path d="M12 5v14M5 12h14"/>');
        button('Zoom out', () => scale(0.8), navigation, '<path d="M5 12h14"/>');
        button(
            'Reset view',
            () => {
                ({ zoom, panX, panY, yaw, pitch } = initial);
            },
            navigation,
            '<path d="M4 10a8 8 0 1 1 1 8M4 4v6h6"/>',
        );
        const announce = (text) => {
            fig.querySelector('.su-diagram-status')?.remove();
            const status = document.createElement('span');
            status.className = 'su-diagram-status';
            status.setAttribute('role', 'status');
            status.textContent = text;
            fig.appendChild(status);
            setTimeout(() => status.remove(), 2200);
        };
        if (stl && fig.__suSourceRange)
            button(
                'Save current view to source',
                () => {
                    fig.dispatchEvent(new CustomEvent('su-save-model-view', { bubbles: true, detail: currentView() }));
                },
                actions,
                '<path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2Z"/><path d="M7 3v6h10V4M7 21v-8h10v8"/>',
            );
        button(
            'Copy source',
            async () => {
                try {
                    await navigator.clipboard.writeText(source);
                    announce('Copied to clipboard');
                } catch {
                    announce('Failed to copy');
                }
            },
            actions,
            '<rect x="8" y="8" width="12" height="13" rx="2"/><path d="M5 16H4V3h12v2"/>',
        );
        button(
            'Toggle viewer fullscreen',
            async () => {
                try {
                    if (document.fullscreenElement === fig) await document.exitFullscreen();
                    else await fig.requestFullscreen();
                } catch {
                    announce('Fullscreen is unavailable');
                }
            },
            actions,
            '<path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5"/>',
        );
        if (stl) {
            const modes = [];
            for (const label of ['Solid', 'Wireframe', 'Surface angle']) {
                const btn = button(
                    label,
                    () => {
                        mode = label;
                        for (const item of modes) item.setAttribute('aria-pressed', String(item.textContent === mode));
                    },
                    modesGroup,
                );
                btn.setAttribute('aria-pressed', String(label === mode));
                modes.push(btn);
            }
        }
        const move = (dx, dy) => {
            if (stl) {
                yaw += dx / 100;
                pitch += dy / 100;
            } else {
                panX += dx;
                panY += dy;
            }
            draw();
        };
        const pan = (dx, dy) => {
            panX += dx;
            panY += dy;
            draw();
        };
        if (stl) {
            const panControls = document.createElement('div');
            panControls.className = 'su-diagram-pan';
            panControls.setAttribute('role', 'group');
            panControls.setAttribute('aria-label', 'Pan model');
            navigation.prepend(panControls);
            button('Pan up', () => pan(0, -20), panControls, '<path d="m5 14 7-7 7 7"/>');
            button('Pan left', () => pan(-20, 0), panControls, '<path d="m14 5-7 7 7 7"/>');
            button('Pan right', () => pan(20, 0), panControls, '<path d="m10 5 7 7-7 7"/>');
            button('Pan down', () => pan(0, 20), panControls, '<path d="m5 10 7 7 7-7"/>');
            button('Rotate left', () => move(-20, 0), rotation, '<path d="m14 5-7 7 7 7"/>');
            button('Rotate right', () => move(20, 0), rotation, '<path d="m10 5 7 7-7 7"/>');
            button('Rotate up', () => move(0, -20), rotation, '<path d="m5 14 7-7 7 7"/>');
            button('Rotate down', () => move(0, 20), rotation, '<path d="m5 10 7 7 7-7"/>');
            button(
                'Reset rotation',
                () => {
                    yaw = initial.yaw;
                    pitch = initial.pitch;
                },
                rotation,
                '<path d="M4 10a8 8 0 1 1 1 8M4 4v6h6"/>',
            );
        }
        let pointer;
        svg.addEventListener('pointerdown', (event) => {
            if (event.button !== 0) return;
            pointer = [event.clientX, event.clientY];
            svg.setPointerCapture(event.pointerId);
        });
        svg.addEventListener('pointermove', (event) => {
            if (!pointer) return;
            const bounds = svg.getBoundingClientRect(),
                ratio = Math.max(800 / bounds.width, 400 / bounds.height);
            const change = stl && event.shiftKey ? pan : move;
            change((event.clientX - pointer[0]) * ratio, (event.clientY - pointer[1]) * ratio);
            pointer = [event.clientX, event.clientY];
        });
        svg.addEventListener('lostpointercapture', () => {
            pointer = null;
        });
        svg.addEventListener('keydown', (event) => {
            const arrows = { ArrowLeft: [-20, 0], ArrowRight: [20, 0], ArrowUp: [0, -20], ArrowDown: [0, 20] };
            if (arrows[event.key]) {
                event.preventDefault();
                const change = stl && event.shiftKey ? pan : move;
                change(...arrows[event.key]);
            } else if (['+', '=', '-'].includes(event.key)) {
                event.preventDefault();
                scale(event.key === '-' ? 0.8 : 1.25);
                draw();
            }
        });
        fig.prepend(svg);
        const details = fig.querySelector('details');
        if (!stl) fig.insertBefore(caption, details);
        fig.insertBefore(controls, details);
        draw();
        // A canvas does not serialize. A detached figure is headed for export, so it stores the
        // first frame as an image.
        if (large && !fig.isConnected) {
            fig.querySelector('.su-stl-snapshot')?.remove();
            const snapshot = document.createElement('img');
            snapshot.className = 'su-stl-snapshot';
            snapshot.alt = 'STL model';
            snapshot.src = svg.toDataURL('image/png');
            fig.appendChild(snapshot);
        }
        if (large) {
            let frame, resize;
            const printSnapshot = () => {
                draw();
                let snapshot = fig.querySelector('.su-stl-snapshot');
                if (!snapshot) {
                    snapshot = document.createElement('img');
                    snapshot.className = 'su-stl-snapshot';
                    snapshot.alt = colored ? '3MF model' : 'STL model';
                    fig.appendChild(snapshot);
                }
                snapshot.src = svg.toDataURL('image/png');
            };
            // Print shows the snapshot, not the canvas. Refresh it with the current view first.
            window.addEventListener('beforeprint', printSnapshot);
            if (fig.isConnected) {
                resize = new ResizeObserver(() => {
                    cancelAnimationFrame(frame);
                    frame = requestAnimationFrame(draw);
                });
                resize.observe(svg);
            }
            fig.__suDispose = () => {
                window.removeEventListener('beforeprint', printSnapshot);
                cancelAnimationFrame(frame);
                resize?.disconnect();
                svg.getContext('webgl')?.getExtension('WEBGL_lose_context')?.loseContext();
            };
        }
    }
}
