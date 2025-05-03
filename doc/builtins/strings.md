## String Manipulation Functions

### `strsub`

**Description:** Substitutes all occurrences of one string with another within a given string.
**Arguments:**

- : The string to perform substitutions on `subject`
- : The substring to find and replace `what`
- : The replacement string `with`
- : Optional boolean (default: false) indicating if case sensitivity should be used `case-matters`

**Returns:** A new string with all substitutions made
If is an empty string, the original string is returned unchanged. **Note:** `what`

### `index`

**Description:** Finds the first occurrence of a substring within a string.
**Arguments:**

- : The string to search in `subject`
- : The substring to find `what`
- : Optional boolean (default: false) indicating if case sensitivity should be used `case-matters`

**Returns:** The 1-based position of the first occurrence, or 0 if not found

### `rindex`

**Description:** Finds the last occurrence of a substring within a string.
**Arguments:**

- : The string to search in `subject`
- : The substring to find `what`
- : Optional boolean (default: false) indicating if case sensitivity should be used `case-matters`

**Returns:** The 1-based position of the last occurrence, or 0 if not found

### `strcmp`

**Description:** Compares two strings lexicographically.
**Arguments:**

- : First string to compare `str1`
- : Second string to compare `str2`

**Returns:** An integer less than, equal to, or greater than 0 if is lexicographically less than, equal to, or greater
than `str1`str2``

## Encryption and Hashing Functions

### `salt`

**Description:** Generates a random cryptographically secure salt string.
**Arguments:** None
**Returns:** A random salt string suitable for use with `argon2`

### `crypt`

**Description:** Encrypts text using the standard UNIX encryption method.
**Arguments:**

- : The text to encrypt `text`
- : Optional salt string (default: random 2-character alphanumeric string) `salt`

**Returns:** The encrypted string, which includes the salt as its first characters

### `string_hash`

**Description:** Computes an MD5 hash of a string.
**Arguments:**

- : The string to hash `text`

**Returns:** The MD5 hash as an uppercase hexadecimal string

### `argon2`

**Description:** Hashes a password using the Argon2id algorithm.
**Arguments:**

- : The password to hash `password`
- : The salt string to use `salt`
- : Optional number of iterations (default: 3) `iterations`
- : Optional memory cost in KB (default: 4096) `memory`
- : Optional parallelism factor (default: 1) `parallelism`

**Returns:** The hashed password string in PHC format
Requires wizard permissions. **Note:**

### `argon2_verify`

**Description:** Verifies a password against an Argon2 hash.
**Arguments:**

- : The previously generated hash `hashed_password`
- : The password to verify `password`

**Returns:** A boolean indicating if the password matches the hash
Requires wizard permissions. **Note:**

## Encoding Functions

### `encode_base64`

**Description:** Encodes a string using Base64 encoding.
**Arguments:**

- : The string to encode `text`

**Returns:** The Base64-encoded string

### `decode_base64`

**Description:** Decodes a Base64-encoded string.
**Arguments:**

- : The Base64-encoded string to decode `encoded_text`

**Returns:** The decoded string
Raises E_INVARG if the input is not valid Base64 or not valid UTF-8. **Note:**

## JSON Functions

### `generate_json`

**Description:** Converts a MOO value to a JSON string.
**Arguments:**

- : The MOO value to convert `value`

**Returns:** A JSON string representation of the value
Supports MOO integers, floats, strings, objects, lists, and maps. Objects are converted to strings in the format "
#object-number". **Note:**

### `parse_json`

**Description:** Parses a JSON string into a MOO value.
**Arguments:**

- : The JSON string to parse `json_str`

**Returns:** The MOO value represented by the JSON
JSON null becomes MOO none, true/false become 1/0, numbers become integers or floats, strings become strings, arrays
become lists, and objects become maps with string keys. **Note:**

## Type Conversion Notes

When converting between MOO and JSON:

- MOO objects are represented as strings like "#123" in JSON
- MOO lists correspond to JSON arrays
- MOO maps correspond to JSON objects (with string keys)
- JSON null converts to MOO none
- JSON booleans convert to MOO integers (1 for true, 0 for false)

## Security Notes

- The and functions require wizard permissions since they involve sensitive cryptographic operations
  `argon2`argon2_verify``
- The function generates cryptographically secure random values suitable for password hashing `salt`
- When storing passwords, use rather than the older function for better security `argon2`crypt``
