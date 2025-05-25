## Map Manipulation Functions

### `mapdelete`

**Description:** Returns a copy of a map with the specified key-value pair removed.
**Arguments:**

- : The map to modify `map`
- : The key to remove from the map `key`

**Returns:** A new map with the specified key removed
**Note:**

- Raises E_TYPE if the first argument is not a map or if the key is a map/list
- Raises E_RANGE if the key does not exist in the map

### `mapkeys`

**Description:** Returns a list containing all keys in a map.
**Arguments:**

- : The map from which to extract keys `map`

**Returns:** A list containing all keys in the map
**Note:**

- Raises E_TYPE if the argument is not a map
- The order of keys in the returned list is not guaranteed to be stable

### `mapvalues`

**Description:** Returns a list containing all values in a map.
**Arguments:**

- : The map from which to extract values `map`

**Returns:** A list containing all values in the map
**Note:**

- Raises E_TYPE if the argument is not a map
- The order of values in the returned list corresponds to the order of keys returned by `mapkeys`

### `maphaskey`

**Description:** Checks if a key exists in a map.
**Arguments:**

- : The map to check `map`
- : The key to look for `key`

**Returns:** A boolean value (true if the key exists, false otherwise)
**Note:**

- Raises E_TYPE if the first argument is not a map or if the key is a map/list
- Performs a case-sensitive key comparison

Note: Map functions in this system work with immutable data structures. Operations like return new maps rather than
modifying the original. Map keys cannot be complex structures like lists or other maps. `mapdelete`
