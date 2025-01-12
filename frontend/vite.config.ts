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
import { MoorHost } from './src/host';

// https://vitejs.dev/config/
export default defineConfig({
	plugins: [react()]
});

declare global {
	var host: MoorHost;
}

// I'm sure this isn't the right 'vite' way to do this, but it works for now. I suspect I'm
// meant to write a plugin or something.
if (!global.host) {
	const verifyingKey = '../moor-verifying-key.pem';
	const signingKey = '../moor-signing-key.pem';
	const daemonRpcAddr = "ipc:///tmp/moor_rpc.sock";
	const daemonEventsAddr = "ipc:///tmp/moor_events.sock";

	global.host = new MoorHost(signingKey, verifyingKey, daemonRpcAddr, daemonEventsAddr);
}