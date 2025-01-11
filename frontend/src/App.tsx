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

import { useEffect, useState, useRef } from "react";
import "./App.css";

const WelcomeMessage = ({ message }) => {
	return <div>{message}</div>;
}

const Messages = ({ messages }) => {
	const messagesEndRef = useRef(null);
	const scrollToBottom = () => {
		messagesEndRef.current.scrollIntoView({ behavior: "smooth" });
	};
	useEffect(scrollToBottom, [messages]);

	return (
		<div className="messages">
			{messages.map(message => (
				<div key={message}>{message}</div>
			))}
			<div ref={messagesEndRef} />
		</div>
	);
};

const InputField = ({ setMessage, sendMessage }) => {
	const inputRef = useRef(null);
	const onEntry = () => {
		inputRef.current.focus();
		sendMessage();
		inputRef.current.value = "";
	};
	return (
		<div className="inputArea">
			<input
				onChange={(e) => setMessage(e.target.value)}
				type="text"
				ref={inputRef}
				placeholder="message"
				className="inputField"
				onKeyDown={(e) => e.key === "Enter" && onEntry()}
			/>
			<button onClick={onEntry} className="inputSubmitButton" type="button">
				Send
			</button>
		</div>
	);
}
function App() {
	const [messages, setMessages] = useState([]);
	const [socket, setSocket] = useState(null);
	const [message, setMessage] = useState("");

	function sendMessage() {
		socket.send(
			JSON.stringify({
				type: "input",
				payload: { message },
			})
		);
	}

	useEffect(() => {
		// On mount, connect to the websocket server, and set up the message handler.
		// TODO: we instead want to start with a login screen, and only connect to the websocket server after the user has logged in.
		//   we will have to add restful endpoints to request login screen, and to submit login credentials, which
		//   will then cause the websocket connection to be established, but with the right auth-key magic.
		//	 all of this was previous implemented in the web-host, but we will have to move it around to the frontend / node-host
		const ws = new WebSocket("ws://localhost:8080");
		ws.onmessage = (event) => setMessages((prev) => [...prev, event.data]);
		ws.onopen = () => {
			ws.send(
				JSON.stringify({
					type: "connect",
					payload: { player: "wizard", password: "" },
				})
			);
		};
		setSocket(ws);

		return () => ws.close();
	}, []);

	return (
		<div>
			<Messages messages={messages} />
			<InputField setMessage={setMessage} sendMessage={sendMessage} />
		</div>
	);
}


export default App;