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

### `age_passphrase_encrypt`

```
BYTES encrypted_message = age_passphrase_encrypt(STR message, STR passphrase)
```
Encrypts the given message using a passphrase (scrypt-based key derivation). Returns the encrypted message as bytes. Note that scrypt is intentionally slow (typically 1-3 seconds) to resist brute-force attacks.

**Example:**
```
;age_passphrase_encrypt("secret data", "my passphrase")
=> b"base-64-data-here"
```

### `age_passphrase_decrypt`

```
STR data = age_passphrase_decrypt(BYTES|STR encrypted_data, STR passphrase)
```
Decrypts an age-encrypted message using a passphrase. Encrypted data can be provided as bytes or base-64 encoded string. Note that scrypt is intentionally slow (typically 1-3 seconds) to resist brute-force attacks.

**Example:**
```
;age_passphrase_decrypt(encrypted, "my passphrase")
=> "secret data"
```

### `paseto_make_local`

```
str paseto_make_local(map|list claims [, str|bytes signing_key])
```

Creates a PASETO V4.Local token from the provided claims map or alist. If `signing_key` is omitted, the server
uses its configured symmetric key (wizard-only). If `signing_key` is provided, any programmer may call it.

The `signing_key` must be a 32-byte binary value or a base64-encoded string.

**Example:**

```
token = paseto_make_local(["sub" -> "player:#123", "role" -> "wizard"]);
```

### `paseto_verify_local`

```
map paseto_verify_local(str token [, str|bytes signing_key])
```

Verifies and decrypts a PASETO V4.Local token and returns the claims as a map. If `signing_key` is omitted, the server
uses its configured symmetric key (wizard-only). Raises `E_INVARG` if the token is invalid.

**Example:**

```
claims = paseto_verify_local(token);
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

### `random_bytes`

```
binary random_bytes(int count)
```

Generates cryptographically secure random bytes.

**Parameters:**
- `count`: Number of bytes to generate (1-65536)

**Returns:** Random bytes as binary data

**Raises:** `E_INVARG` if count is out of range

**Examples:**

```
// Generate a 20-byte secret for TOTP (can be used directly)
secret = random_bytes(20);
code = totp(secret);

// Encode to Base32 if sharing with authenticator apps
encoded = encode_base32(secret);

// Generate a 32-byte key for encryption
key = random_bytes(32);
```

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
- `algorithm`: Hash algorithm to use. Can be `'sha1` or `'sha256`. Defaults to `'sha256`.
- `binary_output`: If true, returns the raw binary HMAC. If false, returns hex-encoded string. Defaults to false.

**Returns:** Hex-encoded string or binary data depending on `binary_output`

**Examples:**

```
string_hmac("hello", "secret")
=> "88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b"

string_hmac("hello", "secret", 'sha1)
=> "2e0e5e2c72b56b2a8c4d9f9f6c2e8e5d3b7f1a4b"

string_hmac("hello", "secret", 'sha256, 1)
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
- `algorithm`: Hash algorithm to use. Can be `'sha1` or `'sha256`. Defaults to `'sha256`.
- `binary_output`: If true, returns the raw binary HMAC. If false, returns hex-encoded string. Defaults to false.

**Returns:** Hex-encoded string or binary data depending on `binary_output`

**Compatibility Note:** This function takes mooR's native Binary type, NOT ToastStunt's bin-string format. The two are not compatible. If you pass a string instead of binary data, you will receive a clear error message indicating this.

**Examples:**

```
binary_hmac(b"\x00\x01\x02", "secret")
=> "f87a5a8f..."

binary_hmac(b"\x00\x01\x02", "secret", 'sha1)
=> "2e0e5e2c..."

binary_hmac(b"\x00\x01\x02", "secret", 'sha256, 1)
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

### `encode_base32`

```
str encode_base32(str|binary data)
```

Encodes the given string or binary data using Base32 encoding ([RFC 4648](https://datatracker.ietf.org/doc/html/rfc4648)).

**Parameters:**
- `data`: String or binary data to encode

**Returns:** Base32-encoded string with padding

**Examples:**

```
encode_base32("Hello")
=> "JBSWY3DP"

encode_base32("foobar")
=> "MZXW6YTBOI======"
```

### `decode_base32`

```
binary decode_base32(str encoded_text)
```

Decodes a Base32-encoded string to binary data.

**Parameters:**
- `encoded_text`: Base32-encoded string to decode (with or without padding)

**Returns:** Decoded binary data

**Raises:** `E_INVARG` if the input is not valid Base32

**Examples:**

```
decode_base32("JBSWY3DP")
=> b"Hello"

binary_to_str(decode_base32("MZXW6YTBOI======"))
=> "foobar"
```

## One-Time Password Functions

These functions implement industry-standard one-time password algorithms used for two-factor authentication (2FA).

### `hotp`

```
str hotp(str|binary secret, int counter [, int digits])
```

Generates an HMAC-based One-Time Password per [RFC 4226](https://datatracker.ietf.org/doc/html/rfc4226).

HOTP generates a one-time password from a shared secret and a counter value. Each time a password is used, the counter should be incremented. This is the foundation for hardware tokens and some authenticator apps.

**Parameters:**
- `secret`: The shared secret key, either as a Base32-encoded string (common format for TOTP apps) or raw binary data
- `counter`: The counter value (must be non-negative). This should be incremented after each successful authentication.
- `digits`: Number of digits in the output (1-10, default 6)

**Returns:** The OTP as a zero-padded string

**Raises:**
- `E_TYPE` if arguments are wrong type
- `E_INVARG` if secret is invalid Base32, counter is negative, or digits is out of range

**Examples:**

```
// Using Base32-encoded secret (standard format)
hotp("JBSWY3DPEHPK3PXP", 0)
=> "282760"

hotp("JBSWY3DPEHPK3PXP", 1)
=> "996344"

// With 8 digits
hotp("JBSWY3DPEHPK3PXP", 0, 8)
=> "84282760"

// Using raw binary secret
hotp(decode_base32("JBSWY3DPEHPK3PXP"), 0)
=> "282760"
```

### `totp`

```
str totp(str|binary secret [, int time] [, int time_step] [, symbol algorithm] [, int digits])
```

Generates a Time-based One-Time Password per [RFC 6238](https://datatracker.ietf.org/doc/html/rfc6238).

TOTP extends HOTP by using the current time as the counter, divided into time steps (typically 30 seconds). This is the algorithm used by Google Authenticator, Authy, and most authenticator apps.

**Parameters:**
- `secret`: The shared secret key, either as a Base32-encoded string or raw binary data
- `time`: Unix timestamp to use (default: current time). Use `time()` to get the current system time.
- `time_step`: Time step in seconds (default 30). Most authenticator apps use 30 seconds.
- `algorithm`: Hash algorithm symbol - `'sha1`, `'sha256`, or `'sha512` (default `'sha256`). Note: Most authenticator apps use SHA1 for compatibility.
- `digits`: Number of digits in the output (1-10, default 6)

**Returns:** The OTP as a zero-padded string

**Raises:**
- `E_TYPE` if arguments are wrong type
- `E_INVARG` if secret is invalid Base32, time is negative, time_step is not positive, algorithm is invalid, or digits is out of range

**Examples:**

```
// Generate current TOTP with default settings (SHA256)
totp("JBSWY3DPEHPK3PXP")
=> "123456"

// Generate TOTP for current time explicitly
totp("JBSWY3DPEHPK3PXP", time())
=> "123456"

// Use SHA1 for compatibility with most authenticator apps
totp("JBSWY3DPEHPK3PXP", time(), 30, 'sha1)
=> "654321"

// Generate TOTP for a specific timestamp (useful for testing)
totp("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59, 30, 'sha1, 8)
=> "94287082"

// With 60-second time step
totp("JBSWY3DPEHPK3PXP", time(), 60)
=> "789012"
```

**Compatibility Note:** While this implementation defaults to SHA256 for better security, most authenticator apps (Google Authenticator, Authy, etc.) use SHA1. When generating secrets for use with these apps, specify `'sha1` as the algorithm.

**See Also:**
- [RFC 4226 - HOTP](https://datatracker.ietf.org/doc/html/rfc4226) - The underlying HMAC-based algorithm
- [RFC 6238 - TOTP](https://datatracker.ietf.org/doc/html/rfc6238) - Time-based extension specification
