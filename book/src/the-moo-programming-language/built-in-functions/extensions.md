### Extensions

The following functions are unique to mooR and not found in original LambdaMOO:

**XML/HTML Content Management:**

- `xml_parse` - Parse a string containing XML into a tree of flyweight objects
- `to_xml` - Convert a tree of flyweight objects into a string containing XML

**Import/Export of Objects:**

- [`load_object`](../../the-system/object-packaging.md#load_object) - Load an object from objdef format with optional
  conflict detection and resolution options.
- `dump_object` - Takes an object and returns a list of strings representing the object definition in objdef format.

**Flyweights & Symbols (New Types):**

- `toflyweight` - Build a flyweight from a delegate, slots map, and optional contents list
- `flyslots` - Returns the slots on a given flyweight as a map
- `flycontents` - Returns the contents list from a flyweight
- `flyslotset` - Returns a copy of the flyweight with a slot added or updated
- `flyslotremove` - Returns a copy of the flyweight with the given slot removed, if present
- `tosym` - Turns the given value into a Symbol

**Cryptography:**

- `age_generate_keypair` - Generates a new X25519 keypair for use with age encryption
- `age_encrypt` - Encrypts a message using age encryption for one or more recipients, outputs as base64
- `age_decrypt` - Decrypts a base64-encoded age-encrypted message using one or more private keys
- `age_encrypt_with_passphrase` - Encrypts a message using age encryption with a passphrase
- `age_decrypt_with_passphrase` - Decrypts an age-encrypted message using a passphrase

**Administration:**

- `vm_counters` - Performance counters for profiling VM internals
- `bf_counters` - Performance counters for profiling builtin function performance
- `db_counters` - Performance counters for profiling DB performance
- [`function_help`](server.md#function_help) - Returns runtime documentation for builtin functions extracted from
  compiled code

**Task Management:**

- `active_tasks` - Return information about running non-suspended/non-queued tasks
- `wait_task` - Causes the current task to wait for a given task id to complete
- `commit` - Immediately commits data, suspends, then resumes (semantically same as `suspend(0)`)
- `rollback` - Immediately rollbacks all mutations to the DB and aborts the current task

### Functions Borrowed from ToastStunt

The following functions were originally extensions in ToastStunt that have been incorporated into mooR:

- `argon2` - Hashing function for secure password storage
- `argon2_verify` - Verifies a password against an Argon2 hash
- `ftime` - Enhanced time formatting (slight differences from ToastStunt implementation)
- `encode_base64` - Encodes a string using Base64 encoding
- `decode_base64` - Decodes a Base64-encoded string
- `slice` - Extracts a portion of a list
- `generate_json` - Converts a MOO value to a JSON string
- `parse_json` - Parses a JSON string into a MOO value
- `ancestors` - Gets a list of all ancestors of an object
- `descendants` - Gets a list of all descendants of an object
- `isa` - Checks if an object is a descendant of a specified ancestor
- `responds_to` - Checks if an object has a specific verb
- `pcre_match` - Enhanced pattern matching using PCRE regular expressions
- `pcre_replace` - Text replacement using PCRE regular expressions
