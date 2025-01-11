import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react-swc';
import { startupHost, startWebSocketServer } from './src/host';

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [react()]
});

console.log("Good morning");

let host = startupHost();

console.log("host: ", host);

startWebSocketServer(host).then(() => {
	console.log("WebSocket server started");
});

