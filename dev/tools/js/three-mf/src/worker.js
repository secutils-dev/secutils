import { import3mf, parseMesh } from './import.js';

self.onmessage = ({ data }) => {
    try {
        const result = data.source === undefined ? import3mf(new Uint8Array(data.buffer)) : parseMesh(data.source);
        self.postMessage(result, [result.positions.buffer, result.colors.buffer]);
    } catch (error) {
        self.postMessage({ error: error.message ?? String(error) });
    }
};
