import { useEffect, useState } from "react";
import "./App.css";

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
			<div>
				{messages.map((m, index) => (
					<div key={index}>
						{m}
					</div>
				))}
			</div>
			<div>
				<input
					onChange={(e) => setMessage(e.target.value)}
					type="text"
					placeholder="message"
					onKeyDown={(e) => e.key === "Enter" && sendMessage()}
				/>
				<button onClick={sendMessage}>
					Send
				</button>
			</div>
		</div>
	);
}

export default App;