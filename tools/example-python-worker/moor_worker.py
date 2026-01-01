#!/usr/bin/env python3
# Copyright (C) 2026 Ryan Daum <ryan.daum@gmail.com> This program is free
# software: you can redistribute it and/or modify it under the terms of the GNU
# General Public License as published by the Free Software Foundation, version
# 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT
# ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
# FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with
# this program. If not, see <https://www.gnu.org/licenses/>.
#

"""mooR worker protocol implementation.

Connects to the mooR daemon via ZeroMQ and processes work requests using
FlatBuffers for serialization and CURVE for authentication.
"""

import json
import os
import socket
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, List, Optional

import flatbuffers
import zmq

# Import generated FlatBuffer types
from moor_schema.MoorRpc import (
    AttachWorker,
    DaemonToWorkerMessage,
    DaemonToWorkerReply,
    EnrollmentRequest,
    EnrollmentResponse,
    PingWorkers,
    PleaseDie,
    RequestResult,
    WorkerPong,
    WorkerRequest,
    WorkerToDaemonMessage,
)
from moor_schema.MoorRpc.DaemonToWorkerMessageUnion import DaemonToWorkerMessageUnion
from moor_schema.MoorRpc.DaemonToWorkerReplyUnion import DaemonToWorkerReplyUnion
from moor_schema.MoorRpc.WorkerToDaemonMessageUnion import (
    WorkerToDaemonMessageUnion as WorkerMsgUnion,
)
from moor_schema.MoorCommon.Symbol import Symbol
from moor_schema.MoorCommon.Uuid import Uuid as FBUuid
from moor_schema.MoorVar.Var import Var
from moor_schema.MoorVar import VarInt
from moor_schema.MoorVar import VarList
from moor_schema.MoorVar import VarStr
from moor_schema.MoorVar.VarUnion import VarUnion

# Constants
WORKER_BROADCAST_TOPIC = b"workers"


class WorkerError(Exception):
    """Error during worker execution."""

    pass


@dataclass
class CurveKeyPair:
    """A CURVE25519 keypair in Z85-encoded format."""

    secret: str  # Z85-encoded secret key (40 characters)
    public: str  # Z85-encoded public key (40 characters)


@dataclass
class WorkerIdentity:
    """Worker identity and enrollment information."""

    service_uuid: str
    service_type: str
    hostname: str
    daemon_curve_public_key: str
    enrolled_at: str


def generate_curve_keypair() -> CurveKeyPair:
    """Generate a new CURVE25519 keypair.

    Returns:
        CurveKeyPair with Z85-encoded secret and public keys
    """
    public_key, secret_key = zmq.curve_keypair()
    return CurveKeyPair(
        secret=secret_key.decode("utf-8"), public=public_key.decode("utf-8")
    )


def load_or_generate_keypair(data_dir: Path, service_type: str) -> CurveKeyPair:
    """Load or generate worker CURVE keypair.

    Looks for keys at data_dir/{service_type}-curve.{key,pub}
    If not found, generates new keys and saves them.

    Args:
        data_dir: Directory to store keys
        service_type: Type of service (e.g., "python-worker")

    Returns:
        CurveKeyPair with Z85-encoded keys
    """
    secret_path = data_dir / f"{service_type}-curve.key"
    public_path = data_dir / f"{service_type}-curve.pub"

    if secret_path.exists() and public_path.exists():
        print(f"Loading existing {service_type} CURVE keys from {data_dir}")
        return _load_keypair(secret_path, public_path)
    else:
        print(f"Generating new {service_type} CURVE keys")
        keypair = generate_curve_keypair()
        _save_keypair(keypair, secret_path, public_path, service_type)
        print(f"Saved {service_type} CURVE keys to {secret_path} and {public_path}")
        return keypair


def _load_keypair(secret_path: Path, public_path: Path) -> CurveKeyPair:
    """Load keypair from files."""
    secret = _parse_key_file(secret_path.read_text(), "secret")
    public = _parse_key_file(public_path.read_text(), "public")
    return CurveKeyPair(secret=secret, public=public)


def _save_keypair(
    keypair: CurveKeyPair, secret_path: Path, public_path: Path, service_type: str
) -> None:
    """Save keypair to files."""
    data_dir = secret_path.parent
    data_dir.mkdir(parents=True, exist_ok=True)

    from datetime import datetime, timezone

    timestamp = datetime.now(timezone.utc).isoformat()

    secret_content = f"""# mooR {service_type} CURVE Secret Key
# Generated: {timestamp}
secret={keypair.secret}
"""

    public_content = f"""# mooR {service_type} CURVE Public Key
# Generated: {timestamp}
public={keypair.public}
"""

    secret_path.write_text(secret_content)
    public_path.write_text(public_content)

    # Restrict permissions on secret key (Unix only)
    try:
        os.chmod(secret_path, 0o600)
    except (OSError, AttributeError):
        pass  # Windows or permission error


def _parse_key_file(content: str, expected_prefix: str) -> str:
    """Parse a key file with format 'key=value'."""
    for line in content.splitlines():
        line = line.strip()
        if line.startswith("#") or not line:
            continue

        if "=" in line:
            key, value = line.split("=", 1)
            if key.strip() == expected_prefix:
                value = value.strip()
                if len(value) == 40:  # Z85-encoded CURVE keys are always 40 chars
                    return value
                else:
                    raise ValueError(
                        f"Invalid {expected_prefix} key length: expected 40, got {len(value)}"
                    )

    raise ValueError(f"No {expected_prefix} key found in file")


def load_identity(data_dir: Path, service_type: str) -> Optional[WorkerIdentity]:
    """Load worker identity from file.

    Returns None if identity file doesn't exist (worker not yet enrolled).
    """
    identity_path = data_dir / f"{service_type}-identity.json"

    if not identity_path.exists():
        return None

    data = json.loads(identity_path.read_text())
    return WorkerIdentity(
        service_uuid=data["service_uuid"],
        service_type=data["service_type"],
        hostname=data["hostname"],
        daemon_curve_public_key=data["daemon_curve_public_key"],
        enrolled_at=data["enrolled_at"],
    )


def save_identity(
    data_dir: Path,
    service_type: str,
    service_uuid: uuid.UUID,
    hostname: str,
    daemon_public_key: str,
) -> None:
    """Save worker identity to file."""
    data_dir.mkdir(parents=True, exist_ok=True)
    identity_path = data_dir / f"{service_type}-identity.json"

    from datetime import datetime, timezone

    identity = {
        "service_uuid": str(service_uuid),
        "service_type": service_type,
        "hostname": hostname,
        "daemon_curve_public_key": daemon_public_key,
        "enrolled_at": datetime.now(timezone.utc).isoformat(),
    }

    identity_path.write_text(json.dumps(identity, indent=2))
    print(f"Saved {service_type} identity (UUID: {service_uuid}) to {identity_path}")


def build_enrollment_request(
    enrollment_token: str, curve_public_key: str, service_type: str, hostname: str
) -> bytes:
    """Build an EnrollmentRequest FlatBuffer message."""
    builder = flatbuffers.Builder(512)

    token_offset = builder.CreateString(enrollment_token)
    key_offset = builder.CreateString(curve_public_key)
    type_offset = builder.CreateString(service_type)
    host_offset = builder.CreateString(hostname)

    EnrollmentRequest.Start(builder)
    EnrollmentRequest.AddEnrollmentToken(builder, token_offset)
    EnrollmentRequest.AddCurvePublicKey(builder, key_offset)
    EnrollmentRequest.AddServiceType(builder, type_offset)
    EnrollmentRequest.AddHostname(builder, host_offset)
    request_offset = EnrollmentRequest.End(builder)

    builder.Finish(request_offset)
    return bytes(builder.Output())


def enroll_with_daemon(
    enrollment_endpoint: str,
    enrollment_token: str,
    service_type: str,
    data_dir: Path,
) -> tuple[str, uuid.UUID]:
    """Enroll this worker with the daemon.

    Returns the daemon's public key and the assigned service UUID.
    """
    # Generate or load our CURVE keypair
    keypair = load_or_generate_keypair(data_dir, service_type)

    # Get hostname
    hostname = socket.gethostname()

    print(f"Enrolling with daemon at {enrollment_endpoint}")
    print(f"  Service type: {service_type}")
    print(f"  Hostname: {hostname}")

    # Create ZMQ context and REQ socket
    ctx = zmq.Context()
    sock = ctx.socket(zmq.REQ)

    try:
        sock.connect(enrollment_endpoint)

        # Build enrollment request
        request = build_enrollment_request(
            enrollment_token, keypair.public, service_type, hostname
        )

        sock.send(request)

        # Receive response
        response_bytes = sock.recv()

        # Parse response
        response = EnrollmentResponse.EnrollmentResponse.GetRootAs(response_bytes)

        if not response.Success():
            error_msg = response.Error()
            if error_msg:
                error_msg = (
                    error_msg.decode("utf-8")
                    if isinstance(error_msg, bytes)
                    else error_msg
                )
            else:
                error_msg = "Unknown error"
            raise WorkerError(f"Enrollment failed: {error_msg}")

        # Extract daemon public key and service UUID
        daemon_public_key = response.DaemonCurvePublicKey()
        if daemon_public_key:
            daemon_public_key = (
                daemon_public_key.decode("utf-8")
                if isinstance(daemon_public_key, bytes)
                else daemon_public_key
            )
        else:
            raise WorkerError("No daemon public key in response")

        service_uuid_str = response.ServiceUuid()
        if service_uuid_str:
            service_uuid_str = (
                service_uuid_str.decode("utf-8")
                if isinstance(service_uuid_str, bytes)
                else service_uuid_str
            )
        else:
            raise WorkerError("No service UUID in response")

        service_uuid = uuid.UUID(service_uuid_str)

        print(f"Successfully enrolled with daemon, service UUID: {service_uuid}")

        # Save identity to disk
        save_identity(data_dir, service_type, service_uuid, hostname, daemon_public_key)

        return daemon_public_key, service_uuid

    finally:
        sock.close()
        ctx.term()


def ensure_enrolled(
    enrollment_endpoint: str,
    enrollment_token: Optional[str],
    enrollment_token_file: Optional[Path],
    service_type: str,
    data_dir: Path,
) -> tuple[str, uuid.UUID]:
    """Check if already enrolled and return identity, or enroll if needed.

    This is the main entry point for worker startup.
    Retries enrollment with exponential backoff if daemon is not yet ready.
    """
    # Check if we already have an identity
    identity = load_identity(data_dir, service_type)
    if identity:
        print(f"Using existing enrollment (UUID: {identity.service_uuid})")
        return identity.daemon_curve_public_key, uuid.UUID(identity.service_uuid)

    # Not enrolled yet - need enrollment token
    # Priority:
    #   1) explicit token arg
    #   2) token file arg
    #   3) XDG default token file
    #   4) MOOR_ENROLLMENT_TOKEN env var
    token = enrollment_token

    if not token and enrollment_token_file:
        try:
            token = enrollment_token_file.read_text().strip()
        except FileNotFoundError:
            pass

    if not token:
        xdg_config = os.environ.get("XDG_CONFIG_HOME", os.path.expanduser("~/.config"))
        xdg_token_path = Path(xdg_config) / "moor" / "enrollment-token"
        try:
            token = xdg_token_path.read_text().strip()
        except FileNotFoundError:
            pass

    if not token:
        token = os.environ.get("MOOR_ENROLLMENT_TOKEN")

    if not token:
        raise WorkerError(
            "Not enrolled and no enrollment token provided. Either:\n"
            "1. Set MOOR_ENROLLMENT_TOKEN environment variable, or\n"
            "2. Use --enrollment-token-file to specify token file path, or\n"
            "3. Place token in ${XDG_CONFIG_HOME:-$HOME/.config}/moor/enrollment-token"
        )

    # Perform enrollment with retry logic
    retry_delay_ms = 100
    max_retry_delay_ms = 5000
    max_retries = 30

    for attempt in range(1, max_retries + 1):
        try:
            return enroll_with_daemon(
                enrollment_endpoint, token, service_type, data_dir
            )
        except Exception as e:
            if attempt == max_retries:
                raise WorkerError(f"Failed to enroll after {max_retries} retries: {e}")

            print(
                f"Enrollment failed (attempt {attempt}/{max_retries}), "
                f"retrying in {retry_delay_ms}ms: {e}"
            )
            time.sleep(retry_delay_ms / 1000.0)
            retry_delay_ms = min(retry_delay_ms * 2, max_retry_delay_ms)

    raise WorkerError("Enrollment failed")  # Should not reach here


def setup_curve_auth(
    rpc_address: str,
    enrollment_endpoint: str,
    enrollment_token_file: Optional[Path],
    service_type: str,
    data_dir: Path,
) -> Optional[tuple[str, str, str]]:
    """Setup CURVE encryption by enrolling with the daemon and loading keys.

    Returns:
        None if the RPC address uses IPC (no encryption needed)
        Some((client_secret, client_public, server_public)) if using TCP

    All returned keys are Z85-encoded strings.
    """
    # Check if we need CURVE encryption (only for TCP endpoints, not IPC)
    use_curve = rpc_address.startswith("tcp://")

    if not use_curve:
        print("IPC endpoint detected - CURVE encryption disabled")
        return None

    print("TCP endpoint detected - enrolling with daemon and loading CURVE keys")

    # Get enrollment token from environment variable
    enrollment_token = os.environ.get("MOOR_ENROLLMENT_TOKEN")

    # Enroll with daemon
    daemon_public_key, _service_uuid = ensure_enrolled(
        enrollment_endpoint,
        enrollment_token,
        enrollment_token_file,
        service_type,
        data_dir,
    )

    # Load or generate CURVE keypair
    keypair = load_or_generate_keypair(data_dir, service_type)

    return (keypair.secret, keypair.public, daemon_public_key)


def build_uuid(builder: flatbuffers.Builder, uid: uuid.UUID) -> int:
    """Build a FlatBuffer UUID."""
    from moor_schema.MoorCommon import Uuid as FBUuidModule

    uuid_data = builder.CreateByteVector(uid.bytes)
    FBUuidModule.UuidStart(builder)
    FBUuidModule.UuidAddData(builder, uuid_data)
    return FBUuidModule.UuidEnd(builder)


def build_symbol(builder: flatbuffers.Builder, value: str) -> int:
    """Build a FlatBuffer Symbol."""
    from moor_schema.MoorCommon import Symbol as SymbolModule

    str_offset = builder.CreateString(value)
    SymbolModule.SymbolStart(builder)
    SymbolModule.SymbolAddValue(builder, str_offset)
    return SymbolModule.SymbolEnd(builder)


def build_attach_worker_message(worker_id: uuid.UUID, worker_type: str) -> bytes:
    """Build a WorkerToDaemonMessage with AttachWorker."""
    builder = flatbuffers.Builder(512)

    uuid_offset = build_uuid(builder, worker_id)
    symbol_offset = build_symbol(builder, worker_type)

    AttachWorker.Start(builder)
    AttachWorker.AddWorkerId(builder, uuid_offset)
    AttachWorker.AddWorkerType(builder, symbol_offset)
    attach_offset = AttachWorker.End(builder)

    WorkerToDaemonMessage.Start(builder)
    WorkerToDaemonMessage.AddMessageType(builder, WorkerMsgUnion.AttachWorker)
    WorkerToDaemonMessage.AddMessage(builder, attach_offset)
    message_offset = WorkerToDaemonMessage.End(builder)

    builder.Finish(message_offset)
    return bytes(builder.Output())


def build_worker_pong_message(worker_id: uuid.UUID, worker_type: str) -> bytes:
    """Build a WorkerToDaemonMessage with WorkerPong."""
    builder = flatbuffers.Builder(512)

    uuid_offset = build_uuid(builder, worker_id)
    symbol_offset = build_symbol(builder, worker_type)

    WorkerPong.Start(builder)
    WorkerPong.AddWorkerId(builder, uuid_offset)
    WorkerPong.AddWorkerType(builder, symbol_offset)
    pong_offset = WorkerPong.End(builder)

    WorkerToDaemonMessage.Start(builder)
    WorkerToDaemonMessage.AddMessageType(builder, WorkerMsgUnion.WorkerPong)
    WorkerToDaemonMessage.AddMessage(builder, pong_offset)
    message_offset = WorkerToDaemonMessage.End(builder)

    builder.Finish(message_offset)
    return bytes(builder.Output())


def build_request_result_message(
    worker_id: uuid.UUID, request_id: uuid.UUID, result_var_offset: int, builder: flatbuffers.Builder
) -> bytes:
    """Build a WorkerToDaemonMessage with RequestResult."""
    worker_uuid_offset = build_uuid(builder, worker_id)
    request_uuid_offset = build_uuid(builder, request_id)

    RequestResult.Start(builder)
    RequestResult.AddWorkerId(builder, worker_uuid_offset)
    RequestResult.AddRequestId(builder, request_uuid_offset)
    RequestResult.AddResult(builder, result_var_offset)
    result_offset = RequestResult.End(builder)

    WorkerToDaemonMessage.Start(builder)
    WorkerToDaemonMessage.AddMessageType(builder, WorkerMsgUnion.RequestResult)
    WorkerToDaemonMessage.AddMessage(builder, result_offset)
    message_offset = WorkerToDaemonMessage.End(builder)

    builder.Finish(message_offset)
    return bytes(builder.Output())


class MoorWorker:
    """Base class for mooR workers.

    Handles connection to daemon, message processing, and response handling via
    ZeroMQ sockets using the FlatBuffers protocol with CURVE authentication.
    """

    def __init__(
        self,
        worker_id: uuid.UUID,
        worker_type: str,
        request_address: str,
        response_address: str,
        curve_keys: Optional[tuple[str, str, str]] = None,
    ):
        """Initialize the worker.

        Args:
            worker_id: Unique worker ID
            worker_type: Type of worker (e.g., "echo", "http")
            request_address: ZMQ address for receiving requests (SUB socket)
            response_address: ZMQ address for sending responses (REQ socket)
            curve_keys: Optional tuple of (client_secret, client_public, server_public)
                       for CURVE encryption. If None, CURVE is disabled.
        """
        self.worker_id = worker_id
        self.worker_type = worker_type
        self.request_address = request_address
        self.response_address = response_address
        self.curve_keys = curve_keys

        self.context = zmq.Context()
        self.response_socket = None
        self.request_socket = None

    def _configure_curve(self, socket: zmq.Socket) -> None:
        """Configure CURVE encryption on a socket if keys are available."""
        if self.curve_keys:
            client_secret, client_public, server_public = self.curve_keys
            socket.curve_secretkey = client_secret.encode("utf-8")
            socket.curve_publickey = client_public.encode("utf-8")
            socket.curve_serverkey = server_public.encode("utf-8")
            print("CURVE encryption enabled for socket")

    def attach(self):
        """Attach to the daemon by sending AttachWorker message."""
        print(f"Creating REQ socket to {self.response_address}...")
        self.response_socket = self.context.socket(zmq.REQ)
        self.response_socket.setsockopt(zmq.RCVTIMEO, 5000)
        self.response_socket.setsockopt(zmq.SNDTIMEO, 5000)
        self.response_socket.setsockopt(zmq.LINGER, 0)
        self._configure_curve(self.response_socket)
        self.response_socket.connect(self.response_address)
        print(f"Connected to {self.response_address}")

        attach_msg = build_attach_worker_message(self.worker_id, self.worker_type)
        worker_id_bytes = self.worker_id.bytes

        print(f"Attaching worker {self.worker_id} of type '{self.worker_type}'...")
        print(f"Sending multipart message:")
        print(f"  Part 1: Worker ID ({len(worker_id_bytes)} bytes)")
        print(f"  Part 2: FlatBuffer ({len(attach_msg)} bytes)")

        self.response_socket.send_multipart([worker_id_bytes, attach_msg])
        print(f"Message sent, waiting for response...")

        reply = self.response_socket.recv()

        # Parse the reply to check for success
        try:
            reply_msg = DaemonToWorkerReply.DaemonToWorkerReply.GetRootAs(reply)
            reply_type = reply_msg.ReplyType()

            if reply_type == DaemonToWorkerReplyUnion.WorkerAttached:
                print(f"Attached successfully!")
            elif reply_type == DaemonToWorkerReplyUnion.WorkerRejected:
                from moor_schema.MoorRpc.WorkerRejected import WorkerRejected
                rejected = WorkerRejected()
                rejected.Init(reply_msg.Reply().Bytes, reply_msg.Reply().Pos)
                reason = rejected.Reason()
                if reason:
                    reason = reason.decode("utf-8") if isinstance(reason, bytes) else reason
                raise WorkerError(f"Worker rejected: {reason}")
            elif reply_type == DaemonToWorkerReplyUnion.WorkerAuthFailed:
                from moor_schema.MoorRpc.WorkerAuthFailed import WorkerAuthFailed
                auth_failed = WorkerAuthFailed()
                auth_failed.Init(reply_msg.Reply().Bytes, reply_msg.Reply().Pos)
                reason = auth_failed.Reason()
                if reason:
                    reason = reason.decode("utf-8") if isinstance(reason, bytes) else reason
                raise WorkerError(f"Worker auth failed: {reason}")
            else:
                print(f"Attached with reply type: {reply_type}")
        except Exception as e:
            if isinstance(e, WorkerError):
                raise
            # If we can't parse, assume success if we got any response
            print(f"Attached successfully (received {len(reply)} bytes)")

    def subscribe(self):
        """Subscribe to worker request channel."""
        self.request_socket = self.context.socket(zmq.SUB)
        self._configure_curve(self.request_socket)
        self.request_socket.connect(self.request_address)
        self.request_socket.setsockopt(zmq.SUBSCRIBE, WORKER_BROADCAST_TOPIC)
        print(f"Subscribed to worker requests at {self.request_address}")

    def _copy_var(self, builder, var):
        """Copy a Var from one FlatBuffer into a new builder.

        Args:
            builder: FlatBuffer builder to copy into
            var: Var object to copy

        Returns:
            Offset of the copied Var in the new builder
        """
        from moor_schema.MoorVar import Var as VarModule

        variant_type = var.VariantType()

        if variant_type == VarUnion.VarStr:
            from moor_schema.MoorVar.VarStr import VarStr as VarStrClass

            varstr = VarStrClass()
            varstr.Init(var.Variant().Bytes, var.Variant().Pos)

            str_val = varstr.Value()
            if str_val:
                if isinstance(str_val, bytes):
                    str_val = str_val.decode("utf-8")
                str_offset = builder.CreateString(str_val)
                VarStr.Start(builder)
                VarStr.AddValue(builder, str_offset)
                varstr_offset = VarStr.End(builder)

                VarModule.Start(builder)
                VarModule.AddVariantType(builder, VarUnion.VarStr)
                VarModule.AddVariant(builder, varstr_offset)
                return VarModule.End(builder)

        elif variant_type == VarUnion.VarInt:
            from moor_schema.MoorVar.VarInt import VarInt as VarIntClass

            varint = VarIntClass()
            varint.Init(var.Variant().Bytes, var.Variant().Pos)

            VarInt.Start(builder)
            VarInt.AddValue(builder, varint.Value())
            varint_offset = VarInt.End(builder)

            VarModule.Start(builder)
            VarModule.AddVariantType(builder, VarUnion.VarInt)
            VarModule.AddVariant(builder, varint_offset)
            return VarModule.End(builder)

        elif variant_type == VarUnion.VarFloat:
            from moor_schema.MoorVar.VarFloat import VarFloat
            from moor_schema.MoorVar.VarFloat import VarFloat as VarFloatClass

            varfloat = VarFloatClass()
            varfloat.Init(var.Variant().Bytes, var.Variant().Pos)

            VarFloat.Start(builder)
            VarFloat.AddValue(builder, varfloat.Value())
            varfloat_offset = VarFloat.End(builder)

            VarModule.Start(builder)
            VarModule.AddVariantType(builder, VarUnion.VarFloat)
            VarModule.AddVariant(builder, varfloat_offset)
            return VarModule.End(builder)

        elif variant_type == VarUnion.VarList:
            from moor_schema.MoorVar.VarList import VarList as VarListClass

            varlist = VarListClass()
            varlist.Init(var.Variant().Bytes, var.Variant().Pos)

            # Recursively copy list elements
            elements = []
            for i in range(varlist.ElementsLength()):
                elem = varlist.Elements(i)
                if elem:
                    elements.append(self._copy_var(builder, elem))

            VarList.StartElementsVector(builder, len(elements))
            for elem_off in reversed(elements):
                builder.PrependUOffsetTRelative(elem_off)
            elements_offset = builder.EndVector()

            VarList.Start(builder)
            VarList.AddElements(builder, elements_offset)
            varlist_offset = VarList.End(builder)

            VarModule.Start(builder)
            VarModule.AddVariantType(builder, VarUnion.VarList)
            VarModule.AddVariant(builder, varlist_offset)
            return VarModule.End(builder)

        # Fallback: return as string placeholder
        str_offset = builder.CreateString(f"<unsupported_type_{variant_type}>")
        VarStr.Start(builder)
        VarStr.AddValue(builder, str_offset)
        varstr_offset = VarStr.End(builder)

        from moor_schema.MoorVar import Var as VarModule

        VarModule.Start(builder)
        VarModule.AddVariantType(builder, VarUnion.VarStr)
        VarModule.AddVariant(builder, varstr_offset)
        return VarModule.End(builder)

    def _build_var_list(self, builder: flatbuffers.Builder, items: List) -> int:
        """Build a VarList containing the given items."""
        from moor_schema.MoorVar import Var as VarModule

        var_offsets = []
        for item in items:
            if isinstance(item, str):
                str_offset = builder.CreateString(item)
                VarStr.Start(builder)
                VarStr.AddValue(builder, str_offset)
                varstr_offset = VarStr.End(builder)

                VarModule.Start(builder)
                VarModule.AddVariantType(builder, VarUnion.VarStr)
                VarModule.AddVariant(builder, varstr_offset)
                var_offsets.append(VarModule.End(builder))
            elif isinstance(item, int):
                VarInt.Start(builder)
                VarInt.AddValue(builder, item)
                varint_offset = VarInt.End(builder)

                VarModule.Start(builder)
                VarModule.AddVariantType(builder, VarUnion.VarInt)
                VarModule.AddVariant(builder, varint_offset)
                var_offsets.append(VarModule.End(builder))
            elif isinstance(item, float):
                from moor_schema.MoorVar.VarFloat import VarFloat

                VarFloat.Start(builder)
                VarFloat.AddValue(builder, item)
                varfloat_offset = VarFloat.End(builder)

                VarModule.Start(builder)
                VarModule.AddVariantType(builder, VarUnion.VarFloat)
                VarModule.AddVariant(builder, varfloat_offset)
                var_offsets.append(VarModule.End(builder))
            elif isinstance(item, int):
                # Already copied Var offset
                var_offsets.append(item)

        VarList.StartElementsVector(builder, len(var_offsets))
        for var_off in reversed(var_offsets):
            builder.PrependUOffsetTRelative(var_off)
        elements_offset = builder.EndVector()

        VarList.Start(builder)
        VarList.AddElements(builder, elements_offset)
        varlist_offset = VarList.End(builder)

        VarModule.Start(builder)
        VarModule.AddVariantType(builder, VarUnion.VarList)
        VarModule.AddVariant(builder, varlist_offset)
        return VarModule.End(builder)

    def _build_request_result(self, request_id_uuid: uuid.UUID, args: List) -> bytes:
        """Build a RequestResult message.

        Args:
            request_id_uuid: UUID of the request
            args: List of Var objects from the request

        Returns:
            Serialized FlatBuffer bytes
        """
        from moor_schema.MoorVar import Var as VarModule

        builder = flatbuffers.Builder(4096)

        # Build result as a list: ["echo_response", ...args]
        # First create the "echo_response" string var
        echo_str = builder.CreateString("echo_response")
        VarStr.Start(builder)
        VarStr.AddValue(builder, echo_str)
        echo_varstr_offset = VarStr.End(builder)

        VarModule.Start(builder)
        VarModule.AddVariantType(builder, VarUnion.VarStr)
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
        VarModule.AddVariantType(builder, VarUnion.VarList)
        VarModule.AddVariant(builder, varlist_offset)
        var_offset = VarModule.End(builder)

        return build_request_result_message(
            self.worker_id, request_id_uuid, var_offset, builder
        )

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

                # Parts: [topic, message]
                message = parts[1]
                daemon_msg = DaemonToWorkerMessage.DaemonToWorkerMessage.GetRootAs(
                    message
                )
                msg_type = daemon_msg.MessageType()

                if msg_type == DaemonToWorkerMessageUnion.PingWorkers:
                    self._handle_ping()

                elif msg_type == DaemonToWorkerMessageUnion.WorkerRequest:
                    self._handle_request(daemon_msg)

                elif msg_type == DaemonToWorkerMessageUnion.PleaseDie:
                    # Check if it's for us
                    please_die = PleaseDie.PleaseDie()
                    please_die.Init(daemon_msg.Message().Bytes, daemon_msg.Message().Pos)
                    worker_id_fb = please_die.WorkerId()
                    if worker_id_fb:
                        worker_id_bytes = bytes(
                            [worker_id_fb.Data(i) for i in range(worker_id_fb.DataLength())]
                        )
                        target_id = uuid.UUID(bytes=worker_id_bytes)
                        if target_id == self.worker_id:
                            print("Received PleaseDie, shutting down...")
                            break
                    else:
                        print("Received PleaseDie (broadcast), shutting down...")
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
        pong_msg = build_worker_pong_message(self.worker_id, self.worker_type)
        worker_id_bytes = self.worker_id.bytes

        self.response_socket.send_multipart([worker_id_bytes, pong_msg])
        self.response_socket.recv()

    def _handle_request(self, daemon_msg):
        """Handle a work request from the daemon.

        Args:
            daemon_msg: DaemonToWorkerMessage containing the request
        """
        worker_req = WorkerRequest.WorkerRequest()
        worker_req.Init(daemon_msg.Message().Bytes, daemon_msg.Message().Pos)

        # Check if this request is for us
        worker_id_fb = worker_req.WorkerId()
        if worker_id_fb:
            worker_id_bytes = bytes(
                [worker_id_fb.Data(i) for i in range(worker_id_fb.DataLength())]
            )
            target_id = uuid.UUID(bytes=worker_id_bytes)
            if target_id != self.worker_id:
                # Not for us
                return
        else:
            print("Error: No worker ID in WorkerRequest")
            return

        # Get request ID
        request_id_fb = worker_req.Id()
        if not request_id_fb:
            print("Error: No request ID in WorkerRequest")
            return

        request_id_bytes = bytes(
            [request_id_fb.Data(i) for i in range(request_id_fb.DataLength())]
        )
        request_id = uuid.UUID(bytes=request_id_bytes)

        print(f"Received work request {request_id}")

        # Get arguments
        args = []
        for i in range(worker_req.RequestLength()):
            arg = worker_req.Request(i)
            if arg:
                args.append(arg)

        print(f"Received {len(args)} arguments")

        # Build and send result
        result_msg = self._build_request_result(request_id, args)
        worker_id_bytes = self.worker_id.bytes

        self.response_socket.send_multipart([worker_id_bytes, result_msg])
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

    parser = argparse.ArgumentParser(description="mooR Python Worker")
    parser.add_argument(
        "--request-address",
        default="ipc:///tmp/moor_workers_request.sock",
        help="ZMQ address for worker requests (SUB socket)",
    )
    parser.add_argument(
        "--response-address",
        default="ipc:///tmp/moor_workers_response.sock",
        help="ZMQ address for worker responses (REQ socket)",
    )
    parser.add_argument(
        "--enrollment-address",
        default="tcp://localhost:7900",
        help="Enrollment server address",
    )
    parser.add_argument(
        "--enrollment-token-file",
        type=Path,
        help="Path to enrollment token file",
    )
    parser.add_argument(
        "--data-dir",
        type=Path,
        default=Path("./.moor-worker-data"),
        help="Directory for worker identity and CURVE keys",
    )
    parser.add_argument("--worker-type", default="echo", help="Worker type")

    args = parser.parse_args()

    # Setup CURVE authentication if using TCP
    curve_keys = setup_curve_auth(
        args.response_address,
        args.enrollment_address,
        args.enrollment_token_file,
        f"python-{args.worker_type}-worker",
        args.data_dir,
    )

    # Generate worker ID
    worker_id = uuid.uuid4()

    print(f"Worker ID: {worker_id}")
    if curve_keys:
        print(f"CURVE keys loaded")

    # Create and run worker
    worker = MoorWorker(
        worker_id=worker_id,
        worker_type=args.worker_type,
        request_address=args.request_address,
        response_address=args.response_address,
        curve_keys=curve_keys,
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


if __name__ == "__main__":
    main()
