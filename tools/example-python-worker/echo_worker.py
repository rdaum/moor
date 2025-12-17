#!/usr/bin/env python3
# Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free
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

"""Echo worker for mooR daemon.

Demonstrates a minimal Python worker that prepends "echo_response" to its
arguments and returns them. Uses CURVE authentication for TCP connections.
"""

import sys
import uuid
from pathlib import Path

from moor_worker import MoorWorker, setup_curve_auth


def echo_process(arguments, timeout):
    """Echo back the arguments with a prepended response marker.

    Args:
        arguments: List of Var arguments from the MOO task
        timeout: Optional timeout duration

    Returns:
        The arguments list unchanged (currently unused by worker)
    """
    print(f"Echo: Received {len(arguments)} arguments")
    return arguments


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Echo worker - prepends echo_response to arguments"
    )
    parser.add_argument(
        "--request-address",
        default="ipc:///tmp/moor_workers_request.sock",
        help="ZMQ address for receiving requests (SUB socket)",
    )
    parser.add_argument(
        "--response-address",
        default="ipc:///tmp/moor_workers_response.sock",
        help="ZMQ address for sending responses (REQ socket)",
    )
    parser.add_argument(
        "--enrollment-address",
        default="tcp://localhost:7900",
        help="Enrollment server address for TCP connections",
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

    args = parser.parse_args()

    # Setup CURVE authentication if using TCP
    curve_keys = setup_curve_auth(
        args.response_address,
        args.enrollment_address,
        args.enrollment_token_file,
        "python-echo-worker",
        args.data_dir,
    )

    worker_id = uuid.uuid4()

    print("Echo Worker")
    print(f"Worker ID: {worker_id}")
    if curve_keys:
        print("CURVE encryption: enabled")
    else:
        print("CURVE encryption: disabled (IPC mode)")
    print("Connecting to daemon...")

    worker = MoorWorker(
        worker_id=worker_id,
        worker_type="echo",
        request_address=args.request_address,
        response_address=args.response_address,
        curve_keys=curve_keys,
    )

    try:
        worker.attach()
        worker.subscribe()
        worker.run(echo_process)
    except KeyboardInterrupt:
        print("\nShutting down echo worker...")
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        import traceback

        traceback.print_exc()
        sys.exit(1)
    finally:
        worker.shutdown()


if __name__ == "__main__":
    main()
