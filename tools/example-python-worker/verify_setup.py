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

"""Verification script for mooR Python worker dependencies."""

import sys


def check_python_version():
    """Verify Python version is 3.10+."""
    print("Checking Python version...", end=" ")

    major, minor = sys.version_info[:2]
    if major >= 3 and minor >= 10:
        print(f"OK ({major}.{minor})")
        return True
    else:
        print(f"FAILED (need 3.10+, got {major}.{minor})")
        return False


def check_imports():
    """Verify all required imports work."""
    print("Checking pyzmq...", end=" ")
    try:
        import zmq
        # Check for CURVE support
        has_curve = hasattr(zmq, 'curve_keypair')
        if has_curve:
            print(f"OK (with CURVE support, zmq {zmq.zmq_version()})")
        else:
            print(f"WARNING (no CURVE support, zmq {zmq.zmq_version()})")
            print("  Note: CURVE is only needed for TCP connections, not IPC")
    except ImportError as e:
        print(f"FAILED: {e}")
        return False

    print("Checking flatbuffers...", end=" ")
    try:
        import flatbuffers
        print("OK")
    except ImportError as e:
        print(f"FAILED: {e}")
        return False

    return True


def check_generated_schemas():
    """Verify generated FlatBuffer schemas can be imported."""
    print("\nChecking generated FlatBuffer schemas...")

    print("  MoorCommon types...", end=" ")
    try:
        from moor_schema.MoorCommon import Symbol, Obj, Uuid
        print("OK")
    except ImportError as e:
        print(f"FAILED: {e}")
        return False

    print("  MoorVar types...", end=" ")
    try:
        from moor_schema.MoorVar import Var
        print("OK")
    except ImportError as e:
        print(f"FAILED: {e}")
        return False

    print("  MoorRpc types...", end=" ")
    try:
        from moor_schema.MoorRpc import (
            AttachWorker,
            WorkerRequest,
            RequestResult,
            EnrollmentRequest,
            EnrollmentResponse,
        )
        print("OK")
    except ImportError as e:
        print(f"FAILED: {e}")
        return False

    return True


def test_curve_keypair():
    """Test CURVE keypair generation."""
    print("\nTesting CURVE keypair generation...", end=" ")

    try:
        import zmq
        public_key, secret_key = zmq.curve_keypair()

        # Verify they're Z85-encoded (40 characters)
        if len(public_key) == 40 and len(secret_key) == 40:
            print("OK")
            return True
        else:
            print(f"FAILED (unexpected key length)")
            return False
    except Exception as e:
        print(f"WARNING: {e}")
        print("  Note: CURVE is only needed for TCP connections, not IPC")
        return True  # Don't fail the overall check


def test_flatbuffer_creation():
    """Test creating a simple FlatBuffer message."""
    print("Testing FlatBuffer message creation...", end=" ")

    try:
        import flatbuffers
        from moor_schema.MoorCommon import Symbol

        builder = flatbuffers.Builder(256)

        # Create a simple Symbol
        test_str = builder.CreateString("test")
        Symbol.SymbolStart(builder)
        Symbol.SymbolAddValue(builder, test_str)
        symbol_offset = Symbol.SymbolEnd(builder)

        builder.Finish(symbol_offset)
        result = bytes(builder.Output())

        print(f"OK ({len(result)} bytes)")
        return True
    except Exception as e:
        print(f"FAILED: {e}")
        return False


def main():
    """Run all verification checks."""
    print("=" * 60)
    print("mooR Python Worker Setup Verification")
    print("=" * 60)
    print()

    all_passed = True

    all_passed = check_python_version() and all_passed
    all_passed = check_imports() and all_passed
    all_passed = check_generated_schemas() and all_passed
    all_passed = test_curve_keypair() and all_passed
    all_passed = test_flatbuffer_creation() and all_passed

    print()
    print("=" * 60)
    if all_passed:
        print("All checks passed! Setup is complete.")
        print()
        print("Next steps:")
        print("  1. Start the mooR daemon with --workers-enabled")
        print("  2. Run: ./run_worker.sh")
        print("     or: python3 echo_worker.py")
        print()
        print("For TCP mode (requires enrollment):")
        print("  export MOOR_ENROLLMENT_TOKEN=<token>")
        print("  WORKER_REQUEST_ADDR=tcp://... ./run_worker.sh")
        return 0
    else:
        print("Some checks failed. Please install missing dependencies:")
        print("  pip install -r requirements.txt")
        print()
        print("If FlatBuffer schemas are missing, regenerate them:")
        print("  cd ../../crates/schema/schema")
        print("  flatc --python -o ../../../tools/example-python-worker/moor_schema/ \\")
        print("      common.fbs var.fbs moor_program.fbs moor_rpc.fbs")
        return 1


if __name__ == "__main__":
    sys.exit(main())
