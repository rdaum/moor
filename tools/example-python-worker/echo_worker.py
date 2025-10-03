#!/usr/bin/env python3
# Copyright (C) 2025 Ryan Daum <ryan.daum@gmail.com> This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, version 3.
#
# This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.
#
"""Echo worker for mooR daemon.

Demonstrates a minimal Python worker that prepends "echo_response" to its
arguments and returns them.
"""

import sys
import uuid
from moor_worker import load_keypair, make_worker_token, MoorWorker


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
        description='Echo worker - prepends echo_response to arguments'
    )
    parser.add_argument(
        '--public-key',
        required=True,
        help='Path to public key PEM file'
    )
    parser.add_argument(
        '--private-key',
        required=True,
        help='Path to private key PEM file'
    )
    parser.add_argument(
        '--request-address',
        default='ipc:///tmp/moor_workers_request.sock',
        help='ZMQ address for receiving requests'
    )
    parser.add_argument(
        '--response-address',
        default='ipc:///tmp/moor_workers_response.sock',
        help='ZMQ address for sending responses'
    )

    args = parser.parse_args()

    try:
        private_key, public_key = load_keypair(args.public_key, args.private_key)
    except Exception as e:
        print(f"Error loading keypair: {e}", file=sys.stderr)
        sys.exit(1)

    worker_id = uuid.uuid4()
    worker_token = make_worker_token(private_key, worker_id)

    print(f"Echo Worker")
    print(f"Worker ID: {worker_id}")
    print(f"Connecting to daemon...")

    worker = MoorWorker(
        worker_id=worker_id,
        worker_type="echo",
        worker_token=worker_token,
        request_address=args.request_address,
        response_address=args.response_address
    )

    try:
        worker.attach()
        worker.subscribe()
        worker.run(echo_process)
    except KeyboardInterrupt:
        print("\nShutting down echo worker...")
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
    finally:
        worker.shutdown()


if __name__ == '__main__':
    main()
