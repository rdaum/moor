# Cryptography and Security Functions

## Encryption / decryption functions
### `age_generate_keypair`

```
{STR public_key, STR private_key} = age_generate_keypair([BOOL as_bytes])
```
Generates a new x25519 keypair for encrypting and decrypting data with [Age](https://age-encryption.org). By default, returns Bech32-encoded strings. If `as_bytes` is true, returns the keys as bytes.

**Example:**
```
;age_generate_keypair()
=> {"age1ac64qtuxuu0acqgsuxdf9kwjgwpxrys4uxtnjclahj8g2y5yv3qswfyfff", "AGE-SECRET-KEY-1TFKKCGX2549ZR7NP2598805NR48Y6GLSA7AZS6X25FSA8U5V5ZES8TU5QM"}
```

### `age_encrypt`

```
BYTES encrypted_message = age_encrypt(STR message, {pubkey...})
```
Encrypts the given message to the public ssh or age keys specified. Public keys can be provided as strings or bytes. Raises an error if the arguments don't match the spec above, the list is empty or one of the provided pubkeys is invalid.
Returns the encrypted message as bytes.

**Example:**
```
;age_encrypt("secret data", {"age1ac64qtuxuu0acqgsuxdf9kwjgwpxrys4uxtnjclahj8g2y5yv3qswfyfff"})
=> b"YWdlLWVuY3J5cHRpb24ub3JnL3YxCi0+IFgyNTUxOSBpSnpKdWV2ZmY2STlla1JVSkVzemtSQUowbmZCeElJb0ZWbmV1ZVNSUm1FClNPYzkxRnFIMWN4UGdOV0N1c1E2WWEwd0g1NEQ4em9EYnRkOElkeGFlMzQKLT4gVi1ncmVhc2UgRHcgVigoICh9YSBnPm0KWkVIaGNvaDhOb1ZVSGNPY29NMVAvQnM2MFEKLS0tIC9YaU9tUG45TlJKV1pZeG9JTi8xL0IwYUJlOFRnTDE2VXhBbG44OEJsVzAKy+j0sHiugXI/N8hk9FTg9Kt+JtWgP/LwUbFR6CROjndmhN+Z2TyWsy6/eQ=="
```

### `age_decrypt`

```
STR data = age_decrypt(BYTES|STR encrypted_data, {private-key...})
```
Given an encrypted message as bytes (such as from age_encrypt) or base-64 encoded string, and one or more private keys, attempt decryption of the data. Private keys can be provided as strings or bytes.

**Example:**
```
;age_decrypt(b"YWdlLWVuY3J5cHRpb24ub3JnL3YxCi0+IFgyNTUxOSBpSnpKdWV2ZmY2STlla1JVSkVzemtSQUowbmZCeElJb0ZWbmV1ZVNSUm1FClNPYzkxRnFIMWN4UGdOV0N1c1E2WWEwd0g1NEQ4em9EYnRkOElkeGFlMzQKLT4gVi1ncmVhc2UgRHcgVigoICh9YSBnPm0KWkVIaGNvaDhOb1ZVSGNPY29NMVAvQnM2MFEKLS0tIC9YaU9tUG45TlJKV1pZeG9JTi8xL0IwYUJlOFRnTDE2VXhBbG44OEJsVzAKy+j0sHiugXI/N8hk9FTg9Kt+JtWgP/LwUbFR6CROjndmhN+Z2TyWsy6/eQ==", {"AGE-SECRET-KEY-1TFKKCGX2549ZR7NP2598805NR48Y6GLSA7AZS6X25FSA8U5V5ZES8TU5QM"})
=> "secret data"
```

### `age_encrypt_with_passphrase`

```
BYTES encrypted_message = age_encrypt_with_passphrase(STR message, STR passphrase)
```
Encrypts the given message using a passphrase (scrypt-based key derivation). Returns the encrypted message as bytes. Note that scrypt is intentionally slow (typically 1-3 seconds) to resist brute-force attacks.

**Example:**
```
;age_encrypt_with_passphrase("secret data", "my passphrase")
=> b"base-64-data-here"
```

### `age_decrypt_with_passphrase`

```
STR data = age_decrypt_with_passphrase(BYTES|STR encrypted_data, STR passphrase)
```
Decrypts an age-encrypted message using a passphrase. Encrypted data can be provided as bytes or base-64 encoded string. Note that scrypt is intentionally slow (typically 1-3 seconds) to resist brute-force attacks.

**Example:**
```
;age_decrypt_with_passphrase(encrypted, "my passphrase")
=> "secret data"
```

## Password Hashing Functions

### `salt`

```
str salt()
```

Generates a random cryptographically secure salt string for use with crypt & argon2.

**Example:**

```
salt()  ⇒    "M5pZr3m8N7q9K4v6B"
```

> Note: This function takes no arguments and generates a cryptographically secure salt string. It is not compatible with Toast's two-argument salt function which allows specifying format and input.

### `crypt`

```
str crypt(str text [, str salt])
```

Encrypts the given text using the standard UNIX encryption method.

Encrypts (hashes) the given text using the standard UNIX encryption method. If provided, salt should be a string at least two characters long, and it may dictate a specific algorithm to use. By default, crypt uses the original, now insecure, DES algorithm. ToastStunt specifically includes the BCrypt algorithm (identified by salts that start with "$2a$"), and may include MD5, SHA256, and SHA512 algorithms depending on the libraries used to build the server. The salt used is returned as the first part of the resulting encrypted string.

Aside from the possibly-random input in the salt, the encryption algorithms are entirely deterministic. In particular, you can test whether or not a given string is the same as the one used to produce a given piece of encrypted text; simply extract the salt from the front of the encrypted text and pass the candidate string and the salt to crypt(). If the result is identical to the given encrypted text, then you've got a match.

**Examples:**

```
crypt("foobar", "iB")                               ⇒    "iBhNpg2tYbVjw"
crypt("foobar", "$1$MAX54zGo")                      ⇒    "$1$MAX54zGo$UKU7XRUEEiKlB.qScC1SX0"
crypt("foobar", "$5$s7z5qpeOGaZb")                  ⇒    "$5$s7z5qpeOGaZb$xkxjnDdRGlPaP7Z ... .pgk/pXcdLpeVCYh0uL9"
crypt("foobar", "$5$rounds=2000$5trdp5JBreEM")      ⇒    "$5$rounds=2000$5trdp5JBreEM$Imi ... ckZPoh7APC0Mo6nPeCZ3"
crypt("foobar", "$6$JR1vVUSVfqQhf2yD")              ⇒    "$6$JR1vVUSVfqQhf2yD$/4vyLFcuPTz ... qI0w8m8az076yMTdl0h."
crypt("foobar", "$6$rounds=5000$hT0gxavqSl0L")      ⇒    "$6$rounds=5000$hT0gxavqSl0L$9/Y ... zpCATppeiBaDxqIbAN7/"
crypt("foobar", "$2a$08$dHkE1lESV9KrErGhhJTxc.")    ⇒    "$2a$08$dHkE1lESV9KrErGhhJTxc.QnrW/bHp8mmBl5vxGVUcsbjo3gcKlf6"
```

> Note: The specific set of supported algorithms depends on the libraries used to build the server. Only the BCrypt algorithm, which is distributed with the server source code, is guaranteed to exist. BCrypt is currently mature and well tested, and is recommended for new development when the Argon2 library is unavailable. (See next section).

> Warning: The entire salt (of any length) is passed to the operating system's low-level crypt function. It is unlikely, however, that all operating systems will return the same string when presented with a longer salt. Therefore, identical calls to crypt() may generate different results on different platforms, and your password verification systems will fail. Use a salt longer than two characters at your own risk.

### `argon2`

```
str argon2(str password, str salt [, int iterations] [, int memory_usage_kb] [, int cpu_threads])
```

The function `argon2()` hashes a password using the Argon2id password hashing algorithm. It is parametrized by three optional arguments:

- Time: This is the number of times the hash will get run. This defines the amount of computation required and, as a result, how long the function will take to complete.
- Memory: This is how much RAM is reserved for hashing.
- Parallelism: This is the number of CPU threads that will run in parallel.

The salt for the password should, at minimum, be 16 bytes for password hashing. It is recommended to use the random_bytes() function.

**Examples:**

```
salt = random_bytes(20);
return argon2(password, salt, 3, 4096, 1);
```

### `argon2_verify`

```
int argon2_verify(str hash, str password)
```

Compares password to the previously hashed hash.

Returns 1 if the two match or 0 if they don't.

This is a more secure way to hash passwords than the `crypt()` builtin.

> Note: More information on Argon2 can be found in the [Argon2 Github](https://github.com/P-H-C/phc-winner-argon2).

## Hash Functions

### `value_hash`

```
str value_hash(value)
```

Computes an MD5 hash of a value's literal representation.

Returns an uppercase hexadecimal string representing the MD5 hash of the value's literal representation (as if `toliteral()` were applied first).

> Note: MD5 is cryptographically broken but is included for compatibility. For secure applications, use `string_hash()` with SHA256 or better algorithms.

### `string_hash`

```
str string_hash(str string [, str algorithm] [, int binary])
```

Returns a string encoding the result of applying the SHA256 cryptographically secure hash function to the contents of the string text.

The `algorithm` parameter can be used to specify different hash algorithms. Supported algorithms may include "MD5", "SHA1", "SHA256", "SHA512", etc., depending on the server build.

If `binary` is true, returns the raw binary hash instead of a hex-encoded string.

### `binary_hash`

```
str binary_hash(str bin_string [, str algorithm] [, int binary])
```

Returns a string encoding the result of applying the SHA256 cryptographically secure hash function to the contents of the binary string bin_string.

The `algorithm` parameter can be used to specify different hash algorithms. Supported algorithms may include "MD5", "SHA1", "SHA256", "SHA512", etc., depending on the server build.

If `binary` is true, returns the raw binary hash instead of a hex-encoded string.

Note that the MD5 hash algorithm is broken from a cryptographic standpoint, as is SHA1. Both are included for interoperability with existing applications (both are still popular).

All supported hash functions have the property that, if

`string_hash(x) == string_hash(y)`

then, almost certainly,

`equal(x, y)`

This can be useful, for example, in certain networking applications: after sending a large piece of text across a connection, also send the result of applying string_hash() to the text; if the destination site also applies string_hash() to the text and gets the same result, you can be quite confident that the large text has arrived unchanged.

### `string_hmac`

```
str|binary string_hmac(str text, str key [, symbol algorithm] [, bool binary_output])
```

Returns the HMAC (Hash-based Message Authentication Code) of the given string using the provided key.

**Parameters:**
- `text`: The string to compute the HMAC for
- `key`: The secret key to use for the HMAC
- `algorithm`: Hash algorithm to use. Can be `#sha1` or `#sha256`. Defaults to `#sha256`.
- `binary_output`: If true, returns the raw binary HMAC. If false, returns hex-encoded string. Defaults to false.

**Returns:** Hex-encoded string or binary data depending on `binary_output`

**Examples:**

```
string_hmac("hello", "secret")
=> "88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b"

string_hmac("hello", "secret", #sha1)
=> "2e0e5e2c72b56b2a8c4d9f9f6c2e8e5d3b7f1a4b"

string_hmac("hello", "secret", #sha256, 1)
=> b"\x88\xaa\xb3\xed\xe8\xd3\xad\xf9..."
```

### `binary_hmac`

```
str|binary binary_hmac(binary data, str key [, symbol algorithm] [, bool binary_output])
```

Returns the HMAC (Hash-based Message Authentication Code) of the given binary data using the provided key.

**Parameters:**
- `data`: The binary data to compute the HMAC for (mooR's native Binary type)
- `key`: The secret key to use for the HMAC
- `algorithm`: Hash algorithm to use. Can be `#sha1` or `#sha256`. Defaults to `#sha256`.
- `binary_output`: If true, returns the raw binary HMAC. If false, returns hex-encoded string. Defaults to false.

**Returns:** Hex-encoded string or binary data depending on `binary_output`

**Compatibility Note:** This function takes mooR's native Binary type, NOT ToastStunt's bin-string format. The two are not compatible. If you pass a string instead of binary data, you will receive a clear error message indicating this.

**Examples:**

```
binary_hmac(b"\x00\x01\x02", "secret")
=> "f87a5a8f..."

binary_hmac(b"\x00\x01\x02", "secret", #sha1)
=> "2e0e5e2c..."

binary_hmac(b"\x00\x01\x02", "secret", #sha256, 1)
=> b"\xf8\x7a\x5a\x8f..."
```

This can be useful, for example, in applications that need to verify both the integrity of the message (the text or binary data) and the authenticity of the sender (as demonstrated by the possession of the secret key).

## Encoding Functions

### `encode_base64`

```
str encode_base64(str|binary data [, bool url_safe] [, bool no_padding])
```

Encodes the given string or binary data using Base64 encoding.

**Parameters:**
- `data`: String or binary data to encode
- `url_safe`: If true, uses URL-safe Base64 alphabet (- and _ instead of + and /). Defaults to false.
- `no_padding`: If true, omits trailing = padding characters. Defaults to false.

**Returns:** Base64-encoded string

**Examples:**

```
encode_base64("hello world")
=> "aGVsbG8gd29ybGQ="

encode_base64("hello world", 1)
=> "aGVsbG8gd29ybGQ="

encode_base64("hello world", 1, 1)
=> "aGVsbG8gd29ybGQ"
```

### `decode_base64`

```
binary decode_base64(str encoded_text [, bool url_safe])
```

Decodes Base64-encoded string to binary data.

**Parameters:**
- `encoded_text`: Base64-encoded string to decode
- `url_safe`: If true, uses URL-safe Base64 alphabet (- and _ instead of + and /). Defaults to false.

**Returns:** Decoded binary data

**Example:**

```
decode_base64("aGVsbG8gd29ybGQ=")
=> b"hello world"

decode_base64("aGVsbG8gd29ybGQ", 1)
=> b"hello world"
```
