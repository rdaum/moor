// Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
// software: you can redistribute it and/or modify it under the terms of the GNU
// General Public License as published by the Free Software Foundation, version
// 3.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.
//

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

