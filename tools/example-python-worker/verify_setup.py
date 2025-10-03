#!/usr/bin/env python3
"""Verification script for mooR Python worker dependencies."""

import sys


def check_imports():
    """Verify all required imports work."""
    print("Checking imports...")

    try:
        import zmq
        print(f"  ✓ zmq {zmq.zmq_version()}")
    except ImportError as e:
        print(f"  ✗ zmq failed: {e}")
        return False

    try:
        import flatbuffers
        print(f"  ✓ flatbuffers")
    except ImportError as e:
        print(f"  ✗ flatbuffers failed: {e}")
        return False

    try:
        import pyseto
        print(f"  ✓ pyseto")
    except ImportError as e:
        print(f"  ✗ pyseto failed: {e}")
        return False

    try:
        from cryptography.hazmat.primitives import serialization
        print(f"  ✓ cryptography")
    except ImportError as e:
        print(f"  ✗ cryptography failed: {e}")
        return False

    return True


def check_generated_schemas():
    """Verify generated FlatBuffer schemas can be imported."""
    print("\nChecking generated FlatBuffer schemas...")

    try:
        from moor_schema.MoorCommon import Symbol, Obj, Uuid
        print(f"  ✓ MoorCommon types")
    except ImportError as e:
        print(f"  ✗ MoorCommon failed: {e}")
        return False

    try:
        from moor_schema.MoorVar import Var
        print(f"  ✓ MoorVar types")
    except ImportError as e:
        print(f"  ✗ MoorVar failed: {e}")
        return False

    try:
        from moor_schema.MoorRpc import (
            AttachWorker,
            WorkerRequest,
            WorkerAck,
            WorkerError,
        )
        print(f"  ✓ MoorRpc types")
    except ImportError as e:
        print(f"  ✗ MoorRpc failed: {e}")
        return False

    return True


def test_paseto_token():
    """Test PASETO token creation."""
    print("\nTesting PASETO token creation...")

    try:
        import uuid
        import pyseto
        from pyseto import Key
        from cryptography.hazmat.primitives.asymmetric import ed25519
        from cryptography.hazmat.primitives import serialization

        # Generate a test Ed25519 keypair
        private_key = ed25519.Ed25519PrivateKey.generate()

        # Get PEM-encoded private key (pyseto expects PEM format)
        private_pem = private_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption()
        )

        # Create Key object
        key = Key.new(version=4, purpose="public", key=private_pem)

        # Create a token
        worker_id = uuid.uuid4()
        token = pyseto.encode(
            key,
            payload=str(worker_id).encode('utf-8'),
            footer=b"key-id:moor_worker"
        )

        print(f"  ✓ Created PASETO token: {token.decode('utf-8')[:50]}...")
        return True
    except Exception as e:
        print(f"  ✗ PASETO token creation failed: {e}")
        return False


def test_flatbuffer_creation():
    """Test creating a simple FlatBuffer message."""
    print("\nTesting FlatBuffer message creation...")

    try:
        import flatbuffers
        from moor_schema.MoorCommon import Symbol

        builder = flatbuffers.Builder(256)

        # Create a simple Symbol
        test_str = builder.CreateString("test")
        Symbol.Start(builder)
        Symbol.AddValue(builder, test_str)
        symbol_offset = Symbol.End(builder)

        builder.Finish(symbol_offset)
        result = bytes(builder.Output())

        print(f"  ✓ Created FlatBuffer message ({len(result)} bytes)")
        return True
    except Exception as e:
        print(f"  ✗ FlatBuffer creation failed: {e}")
        return False


def main():
    """Run all verification checks."""
    print("=" * 60)
    print("mooR Python Worker Setup Verification")
    print("=" * 60)

    all_passed = True

    all_passed = check_imports() and all_passed
    all_passed = check_generated_schemas() and all_passed
    all_passed = test_paseto_token() and all_passed
    all_passed = test_flatbuffer_creation() and all_passed

    print("\n" + "=" * 60)
    if all_passed:
        print("✓ All checks passed! Setup is complete.")
        print("\nNext steps:")
        print("  1. Generate or obtain keypair files (public_key.pem, private_key.pem)")
        print("  2. Start the mooR daemon")
        print("  3. Run: python3 echo_worker.py --public-key KEY --private-key KEY")
        return 0
    else:
        print("✗ Some checks failed. Please install missing dependencies:")
        print("  pip install -r requirements.txt")
        return 1


if __name__ == '__main__':
    sys.exit(main())
