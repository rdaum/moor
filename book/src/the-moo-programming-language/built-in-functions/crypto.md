# Cryptography and Security Functions

## Password Hashing Functions

### `salt`

```
str salt(str format, str input)
```

Generate a crypt() compatible salt string for the specified salt format using the specified binary random input.

The specific set of formats supported depends on the libraries used to build the server, but will always include the standard salt format, indicated by the format string "" (the empty string), and the BCrypt salt format, indicated by the format string "$2a$NN$" (where "NN" is the work factor). Other possible formats include MD5 ("$1$"), SHA256 ("$5$") and SHA512 ("$6$"). Both the SHA256 and SHA512 formats support optional rounds.

**Examples:**

```
salt("", ".M")                                           ⇒    "iB"
salt("$1$", "~183~1E~C6/~D1")                            ⇒    "$1$MAX54zGo"
salt("$5$", "x~F2~1Fv~ADj~92Y~9E~D4l~C3")                ⇒    "$5$s7z5qpeOGaZb"
salt("$5$rounds=2000$", "G~7E~A7~F5Q5~B7~0Aa~80T")       ⇒    "$5$rounds=2000$5trdp5JBreEM"
salt("$6$", "U7~EC!~E8~85~AB~CD~B5+~E1?")                ⇒    "$6$JR1vVUSVfqQhf2yD"
salt("$6$rounds=5000$", "~ED'~B0~BD~B9~DB^,\\~BD~E7")    ⇒    "$6$rounds=5000$hT0gxavqSl0L"
salt("$2a$08$", "|~99~86~DEq~94_~F3-~1A~D2#~8C~B5sx")    ⇒    "$2a$08$dHkE1lESV9KrErGhhJTxc."
```

> Note: To ensure proper security, the random input must be from a sufficiently random source.

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

> Warning: The MOO is single threaded in most cases, and this function can take significant time depending on how you call it. While it is working, nothing else is going to be happening on your MOO. It is possible to build the server with the `THREAD_ARGON2` option which will mitigate lag. This has major caveats however, see the section below on `argon2_verify` for more information.

### `argon2_verify`

```
int argon2_verify(str hash, str password)
```

Compares password to the previously hashed hash.

Returns 1 if the two match or 0 if they don't.

This is a more secure way to hash passwords than the `crypt()` builtin.

> Warning: It is possible to build the server with the `THREAD_ARGON2` option. This will enable this built-in to run in a background thread and mitigate lag that these functions can cause. However, this comes with some major caveats. `do_login_command` (where you will typically be verifying passwords) cannot be suspended. Since threading implicitly suspends the MOO task, you won't be able to directly use Argon2 in do_login_command. Instead, you'll have to devise a new solution for logins that doesn't directly involve calling Argon2 in do_login_command.

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
str string_hmac(str string, str key [, str algorithm] [, int binary])
```

Returns the HMAC (Hash-based Message Authentication Code) of the given string using the provided key.

The `algorithm` parameter can be used to specify different hash algorithms for the HMAC computation.

If `binary` is true, returns the raw binary HMAC instead of a hex-encoded string.

### `binary_hmac`

```
str binary_hmac(str bin_string, str key [, str algorithm] [, int binary])
```

Returns the HMAC (Hash-based Message Authentication Code) of the given binary string using the provided key.

The `algorithm` parameter can be used to specify different hash algorithms for the HMAC computation.

If `binary` is true, returns the raw binary HMAC instead of a hex-encoded string.

This can be useful, for example, in applications that need to verify both the integrity of the message (the text) and the authenticity of the sender (as demonstrated by the possession of the secret key).
