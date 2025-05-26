## Map Manipulation Functions

When using the functions below, it's helpful to remember that maps are ordered.

### `mapkeys`

```
list mapkeys(map map)
```

returns the keys of the elements of a map.

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapkeys(x)   =>  {"bar", "baz", "foo"}
```

### `mapvalues`

```
list mapvalues(MAP `map` [, ... STR `key`])
```

returns the values of the elements of a map.

If you only want the values of specific keys in the map, you can specify them as optional arguments. See examples below.

Examples:

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapvalues(x)               =>  {2, 3, 1}
mapvalues(x, "foo", "baz") => {1, 3}
```

### `mapdelete`

```
map mapdelete(map map, key)
```

Returns a copy of map with the value corresponding to key removed. If key is not a valid key, then E_RANGE is raised.

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapdelete(x, "bar")   ⇒   ["baz" -> 3, "foo" -> 1]
```

### `maphaskey`

```
int maphaskey(MAP map, STR key)
```

Returns 1 if key exists in map. When not dealing with hundreds of keys, this function is faster (and easier to read)
than something like: !(x in mapkeys(map))
