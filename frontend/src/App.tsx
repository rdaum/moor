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