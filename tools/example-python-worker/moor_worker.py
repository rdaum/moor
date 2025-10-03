#!/usr/bin/env python3
"""mooR worker protocol implementation.

Connects to the mooR daemon via ZeroMQ and processes work requests using
FlatBuffers for serialization and PASETO for authentication.
"""

import uuid
import zmq
import flatbuffers
from typing import Callable, Optional, List, Any
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.backends import default_backend
from pyseto import Key
import pyseto

# Import generated FlatBuffer types
from moor_schema.MoorRpc import (
    AttachWorker,
    DaemonToWorkerMessage,
    DaemonToWorkerMessageUnion,
    WorkerRequest,
    WorkerToDaemonMessage,
    WorkerToDaemonMessageUnion,
    WorkerAck,
    WorkerError as FBWorkerError,
    PingWorkers,
    WorkerPong,
    WorkerToken as FBWorkerToken,
)
from moor_schema.MoorCommon import Symbol, Uuid as FBUuid
from moor_schema.MoorVar import Var

# Constants
MOOR_WORKER_TOKEN_FOOTER = b"key-id:moor_worker"


class WorkerError(Exception):
    """Error during worker execution."""
    pass


def load_keypair(public_key_path: str, private_key_path: str) -> tuple[bytes, bytes]:
    """Load Ed25519 keypair from PEM files.

    Args:
        public_key_path: Path to public key PEM file
        private_key_path: Path to private key PEM file

    Returns:
        Tuple of (private_key_pem, public_key_pem) as bytes

    Raises:
        ValueError: If the keypair files are invalid or unreadable
    """
    with open(private_key_path, 'rb') as f:
        private_pem = f.read()

    with open(public_key_path, 'rb') as f:
        public_pem = f.read()

    try:
        serialization.load_pem_private_key(
            private_pem,
            password=None,
            backend=default_backend()
        )
        serialization.load_pem_public_key(
            public_pem,
            backend=default_backend()
        )
    except Exception as e:
        raise ValueError(f"Invalid keypair: {e}") from e

    return private_pem, public_pem


def make_worker_token(private_key_pem: bytes, worker_id: uuid.UUID) -> str:
    """Create a PASETO v4.public token for worker authentication.

    Args:
        private_key_pem: PEM-encoded Ed25519 private key
        worker_id: Worker's UUID

    Returns:
        PASETO token string
    """
    key = Key.new(version=4, purpose="public", key=private_key_pem)
    token = pyseto.encode(
        key,
        payload=str(worker_id).encode('utf-8'),
        footer=MOOR_WORKER_TOKEN_FOOTER
    )
    return token.decode('utf-8')


def build_attach_worker_message(worker_token: str, worker_type: str) -> bytes:
    """Build a WorkerToDaemonMessage with AttachWorker.

    Args:
        worker_token: PASETO token string
        worker_type: Type of worker (e.g., "echo")

    Returns:
        Serialized FlatBuffer bytes
    """
    from moor_schema.MoorRpc import (
        WorkerToken as FBWorkerToken,
        WorkerToDaemonMessage,
    )
    from moor_schema.MoorRpc.WorkerToDaemonMessageUnion import WorkerToDaemonMessageUnion

    builder = flatbuffers.Builder(512)

    token_str_offset = builder.CreateString(worker_token)
    FBWorkerToken.Start(builder)
    FBWorkerToken.AddToken(builder, token_str_offset)
    token_offset = FBWorkerToken.End(builder)

    type_str_offset = builder.CreateString(worker_type)
    Symbol.Start(builder)
    Symbol.AddValue(builder, type_str_offset)
    symbol_offset = Symbol.End(builder)

    AttachWorker.Start(builder)
    AttachWorker.AddToken(builder, token_offset)
    AttachWorker.AddWorkerType(builder, symbol_offset)
    attach_offset = AttachWorker.End(builder)

    WorkerToDaemonMessage.Start(builder)
    WorkerToDaemonMessage.AddMessageType(builder, WorkerToDaemonMessageUnion.AttachWorker)
    WorkerToDaemonMessage.AddMessage(builder, attach_offset)
    message_offset = WorkerToDaemonMessage.End(builder)

    builder.Finish(message_offset)
    return bytes(builder.Output())


def build_worker_pong_message(worker_token: str, worker_type: str) -> bytes:
    """Build a WorkerToDaemonMessage with WorkerPong.

    Args:
        worker_token: PASETO token string
        worker_type: Type of worker (e.g., "echo")

    Returns:
        Serialized FlatBuffer bytes
    """
    from moor_schema.MoorRpc import (
        WorkerToken as FBWorkerToken,
        WorkerToDaemonMessage,
    )
    from moor_schema.MoorRpc.WorkerToDaemonMessageUnion import WorkerToDaemonMessageUnion

    builder = flatbuffers.Builder(512)

    token_str_offset = builder.CreateString(worker_token)
    FBWorkerToken.Start(builder)
    FBWorkerToken.AddToken(builder, token_str_offset)
    token_offset = FBWorkerToken.End(builder)

    type_str_offset = builder.CreateString(worker_type)
    Symbol.Start(builder)
    Symbol.AddValue(builder, type_str_offset)
    symbol_offset = Symbol.End(builder)

    WorkerPong.Start(builder)
    WorkerPong.AddToken(builder, token_offset)
    WorkerPong.AddWorkerType(builder, symbol_offset)
    pong_offset = WorkerPong.End(builder)

    WorkerToDaemonMessage.Start(builder)
    WorkerToDaemonMessage.AddMessageType(builder, WorkerToDaemonMessageUnion.WorkerPong)
    WorkerToDaemonMessage.AddMessage(builder, pong_offset)
    message_offset = WorkerToDaemonMessage.End(builder)

    builder.Finish(message_offset)
    return bytes(builder.Output())


class MoorWorker:
    """Base class for mooR workers.

    Handles connection to daemon, message processing, and response handling via
    ZeroMQ sockets using the FlatBuffers protocol.
    """

    def __init__(
        self,
        worker_id: uuid.UUID,
        worker_type: str,
        worker_token: str,
        request_address: str,
        response_address: str
    ):
        """Initialize the worker.

        Args:
            worker_id: Unique worker ID
            worker_type: Type of worker (e.g., "echo", "http")
            worker_token: PASETO authentication token
            request_address: ZMQ address for receiving requests (SUB socket)
            response_address: ZMQ address for sending responses (REQ socket)
        """
        self.worker_id = worker_id
        self.worker_type = worker_type
        self.worker_token = worker_token
        self.request_address = request_address
        self.response_address = response_address

        self.context = zmq.Context()
        self.response_socket = None
        self.request_socket = None

    def attach(self):
        """Attach to the daemon by sending AttachWorker message."""
        print(f"Creating REQ socket to {self.response_address}...")
        self.response_socket = self.context.socket(zmq.REQ)
        self.response_socket.connect(self.response_address)
        print(f"Connected to {self.response_address}")

        attach_msg = build_attach_worker_message(self.worker_token, self.worker_type)
        worker_token_bytes = self.worker_token.encode('utf-8')
        worker_id_bytes = self.worker_id.bytes

        print(f"Attaching worker {self.worker_id} of type '{self.worker_type}'...")
        print(f"Sending multipart message:")
        print(f"  Part 1: Token ({len(worker_token_bytes)} bytes)")
        print(f"  Part 2: Worker ID ({len(worker_id_bytes)} bytes)")
        print(f"  Part 3: FlatBuffer ({len(attach_msg)} bytes)")

        self.response_socket.send_multipart([
            worker_token_bytes,
            worker_id_bytes,
            attach_msg
        ])
        print(f"Message sent, waiting for response...")

        reply = self.response_socket.recv()
        print(f"Attached successfully (received {len(reply)} bytes)")

    def subscribe(self):
        """Subscribe to worker request channel."""
        self.request_socket = self.context.socket(zmq.SUB)
        self.request_socket.connect(self.request_address)
        self.request_socket.setsockopt_string(zmq.SUBSCRIBE, "")
        print(f"Subscribed to worker requests at {self.request_address}")

    def _copy_var(self, builder, var):
        """Copy a Var from one FlatBuffer into a new builder.

        Args:
            builder: FlatBuffer builder to copy into
            var: Var object to copy

        Returns:
            Offset of the copied Var in the new builder
        """
        from moor_schema.MoorVar import (
            Var as VarModule, VarStr, VarInt, VarFloat, VarList,
            VarObj, VarErr, VarUnion
        )
        from moor_schema.MoorCommon import Obj as ObjModule

        variant_type = var.VariantType()

        if variant_type == VarUnion.VarUnion.VarStr:
            from moor_schema.MoorVar.VarStr import VarStr as VarStrClass
            varstr = VarStrClass()
            varstr.Init(var.Variant().Bytes, var.Variant().Pos)

            str_val = varstr.Value()
            if str_val:
                str_offset = builder.CreateString(
                    str_val.decode('utf-8') if isinstance(str_val, bytes) else str_val
                )
                VarStr.Start(builder)
                VarStr.AddValue(builder, str_offset)
                varstr_offset = VarStr.End(builder)

                VarModule.Start(builder)
                VarModule.AddVariantType(builder, VarUnion.VarUnion.VarStr)
                VarModule.AddVariant(builder, varstr_offset)
                return VarModule.End(builder)

        elif variant_type == VarUnion.VarUnion.VarInt:
            from moor_schema.MoorVar.VarInt import VarInt as VarIntClass
            varint = VarIntClass()
            varint.Init(var.Variant().Bytes, var.Variant().Pos)

            VarInt.Start(builder)
            VarInt.AddValue(builder, varint.Value())
            varint_offset = VarInt.End(builder)

            VarModule.Start(builder)
            VarModule.AddVariantType(builder, VarUnion.VarUnion.VarInt)
            VarModule.AddVariant(builder, varint_offset)
            return VarModule.End(builder)

        str_offset = builder.CreateString(f"<unsupported_type_{variant_type}>")
        VarStr.Start(builder)
        VarStr.AddValue(builder, str_offset)
        varstr_offset = VarStr.End(builder)

        VarModule.Start(builder)
        VarModule.AddVariantType(builder, VarUnion.VarUnion.VarStr)
        VarModule.AddVariant(builder, varstr_offset)
        return VarModule.End(builder)

    def _build_request_result(self, request_id, args):
        """Build a RequestResult message.

        Args:
            request_id: Uuid object from the request
            args: List of Var objects from the request

        Returns:
            Serialized FlatBuffer bytes
        """
        from moor_schema.MoorRpc import RequestResult
        from moor_schema.MoorRpc.WorkerToDaemonMessageUnion import WorkerToDaemonMessageUnion
        from moor_schema.MoorVar import Var as VarModule, VarStr, VarList, VarUnion

        builder = flatbuffers.Builder(4096)

        uuid_data = bytes([request_id.Data(i) for i in range(request_id.DataLength())])
        uuid_data_offset = builder.CreateByteVector(uuid_data)

        FBUuid.Start(builder)
        FBUuid.AddData(builder, uuid_data_offset)
        uuid_offset = FBUuid.End(builder)

        echo_str = builder.CreateString("echo_response")
        VarStr.Start(builder)
        VarStr.AddValue(builder, echo_str)
        echo_varstr_offset = VarStr.End(builder)

        VarModule.Start(builder)
        VarModule.AddVariantType(builder, VarUnion.VarUnion.VarStr)
        VarModule.AddVariant(builder, echo_varstr_offset)
        echo_var_offset = VarModule.End(builder)

        var_offsets = [echo_var_offset]
        for arg in args:
            copied_var = self._copy_var(builder, arg)
            var_offsets.append(copied_var)

        VarList.StartElementsVector(builder, len(var_offsets))
        for var_off in reversed(var_offsets):
            builder.PrependUOffsetTRelative(var_off)
        elements_offset = builder.EndVector()

        VarList.Start(builder)
        VarList.AddElements(builder, elements_offset)
        varlist_offset = VarList.End(builder)

        VarModule.Start(builder)
        VarModule.AddVariantType(builder, VarUnion.VarUnion.VarList)
        VarModule.AddVariant(builder, varlist_offset)
        var_offset = VarModule.End(builder)

        token_str_offset = builder.CreateString(self.worker_token)
        FBWorkerToken.Start(builder)
        FBWorkerToken.AddToken(builder, token_str_offset)
        token_offset = FBWorkerToken.End(builder)

        RequestResult.Start(builder)
        RequestResult.AddToken(builder, token_offset)
        RequestResult.AddId(builder, uuid_offset)
        RequestResult.AddResult(builder, var_offset)
        result_offset = RequestResult.End(builder)

        WorkerToDaemonMessage.Start(builder)
        WorkerToDaemonMessage.AddMessageType(builder, WorkerToDaemonMessageUnion.RequestResult)
        WorkerToDaemonMessage.AddMessage(builder, result_offset)
        message_offset = WorkerToDaemonMessage.End(builder)

        builder.Finish(message_offset)
        return bytes(builder.Output())

    def run(self, process_func: Callable[[List[Any], Optional[float]], Any]):
        """Main worker loop.

        Receives and processes work requests from the daemon.

        Args:
            process_func: Function to process work requests (currently unused)
        """
        print(f"Worker {self.worker_id} running...")

        while True:
            try:
                parts = self.request_socket.recv_multipart()
                if len(parts) < 2:
                    print(f"Received malformed message with {len(parts)} parts")
                    continue

                message = parts[1]
                daemon_msg = DaemonToWorkerMessage.DaemonToWorkerMessage.GetRootAs(message)
                msg_type = daemon_msg.MessageType()

                if msg_type == DaemonToWorkerMessageUnion.DaemonToWorkerMessageUnion.PingWorkers:
                    self._handle_ping()

                elif msg_type == DaemonToWorkerMessageUnion.DaemonToWorkerMessageUnion.WorkerRequest:
                    self._handle_request(daemon_msg)

                elif msg_type == DaemonToWorkerMessageUnion.DaemonToWorkerMessageUnion.PleaseDie:
                    print("Received PleaseDie, shutting down...")
                    break

                else:
                    print(f"Received unknown message type: {msg_type}")

            except KeyboardInterrupt:
                print("\nShutting down worker...")
                break
            except Exception as e:
                print(f"Error processing request: {e}")
                import traceback
                traceback.print_exc()

    def _handle_ping(self):
        """Respond to daemon ping with pong."""
        print(f"Received ping, sending pong...")
        pong_msg = build_worker_pong_message(self.worker_token, self.worker_type)

        worker_token_bytes = self.worker_token.encode('utf-8')
        worker_id_bytes = self.worker_id.bytes

        self.response_socket.send_multipart([
            worker_token_bytes,
            worker_id_bytes,
            pong_msg
        ])
        self.response_socket.recv()

    def _handle_request(self, daemon_msg):
        """Handle a work request from the daemon.

        Args:
            daemon_msg: DaemonToWorkerMessage containing the request
        """
        print(f"Received work request")

        worker_req_table = daemon_msg.Message()
        worker_req = WorkerRequest.WorkerRequest()
        worker_req.Init(worker_req_table.Bytes, worker_req_table.Pos)

        from moor_schema.MoorCommon.Uuid import Uuid as FBUuidClass
        request_id_offset = worker_req._tab.Offset(8)
        if request_id_offset == 0:
            print("Error: No request ID in WorkerRequest")
            return

        request_id_table = worker_req._tab.Indirect(request_id_offset + worker_req._tab.Pos)
        request_id = FBUuidClass()
        request_id.Init(worker_req._tab.Bytes, request_id_table)

        args = []
        for i in range(worker_req.RequestLength()):
            arg = worker_req.Request(i)
            if arg:
                args.append(arg)

        print(f"Received {len(args)} arguments")

        result_msg = self._build_request_result(request_id, args)

        worker_token_bytes = self.worker_token.encode('utf-8')
        worker_id_bytes = self.worker_id.bytes

        self.response_socket.send_multipart([
            worker_token_bytes,
            worker_id_bytes,
            result_msg
        ])
        reply = self.response_socket.recv()
        print(f"Result sent, received ack ({len(reply)} bytes)")

    def shutdown(self):
        """Clean up sockets and context."""
        if self.request_socket:
            self.request_socket.close()
        if self.response_socket:
            self.response_socket.close()
        self.context.term()


def main():
    """Example usage"""
    import argparse

    parser = argparse.ArgumentParser(description='mooR Python Worker')
    parser.add_argument('--public-key', required=True, help='Path to public key PEM file')
    parser.add_argument('--private-key', required=True, help='Path to private key PEM file')
    parser.add_argument('--request-address', default='tcp://localhost:7899',
                       help='ZMQ address for worker requests')
    parser.add_argument('--response-address', default='tcp://localhost:7898',
                       help='ZMQ address for worker responses')
    parser.add_argument('--worker-type', default='echo', help='Worker type')

    args = parser.parse_args()

    # Load keypair
    private_key, public_key = load_keypair(args.public_key, args.private_key)

    # Generate worker ID and token
    worker_id = uuid.uuid4()
    worker_token = make_worker_token(private_key, worker_id)

    print(f"Worker ID: {worker_id}")
    print(f"Token: {worker_token[:50]}...")

    # Create and run worker
    worker = MoorWorker(
        worker_id=worker_id,
        worker_type=args.worker_type,
        worker_token=worker_token,
        request_address=args.request_address,
        response_address=args.response_address
    )

    try:
        worker.attach()
        worker.subscribe()

        # Simple echo function
        def echo_process(arguments: List[Any], timeout: Optional[float]) -> Any:
            return arguments

        worker.run(echo_process)
    finally:
        worker.shutdown()


if __name__ == '__main__':
    main()
