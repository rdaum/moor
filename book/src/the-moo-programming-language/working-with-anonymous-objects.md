## Working with Anonymous Objects

Anonymous objects are typically transient and are garbage collected when they are no longer needed (IE: when nothing is referencing them).

A reference to an anonymous object is returned when the anonymous object is created with create(). References can be shared but they cannot be forged. That is, there is no literal representation of a reference to an anonymous object (that`s why they are anonymous).

Anonymous objects are created using the `create` builtin, passing the optional third argument as `1`. For example:

```
anonymous = create($thing, #2, 1);
```

Since there is no literal representation of an anonymous object, if you were to try to print it:

```
player:tell(toliteral(anonymous));
```

You would be shown: `\*anonymous\*`

You can store the reference to the anonymous object in a variable, like we did above, or you can store it in a property.

```
player.test = create($thing, player, 1)
player:tell(player.test);
```

This will also output: `\*anonymous\*`

If you store your anonymous object in a property, that anonymous object will continue to exist so long as it exists in the property. If the object with the property were recycled, or the property removed or overwritten with a different value, the anonymous object would be garbage collected.

Anonymous objects can be stored in lists:

```
my_list = {create($thing, player, 1)};
player.test = my_list;
```

The above code would result in:

```
{\*anonymous\*}
```

Anonymous objects can be stored in maps as either the key or the value:

```
[1 -> create($thing, player, 1)] => [1 -> \*anonymous\*]
[create($thing, player, 1) -> 1] => [\*anonymous\* -> 1]
```

> Warning: \*anonymous\* is not the actual key, there is not literal representation of an anonymous object reference. This means that while the object will continue to exist while it is a key of a map, the only way to reference that key would be by the reference, which you would need to store in a variable or a property. This is NOT a recommended practice, as you would have to keep a reference to the key elsewhere in order to access it (outside of iterating over all the keys).

Anonymous objects technically have a player flag and children lists, but you can't actually do anything with them. Same with the majority of the properties. They exist but are meaningless. Generally speaking, this makes WAIFs a better choice in most situations, as they are lighter weight.

> Warning: Similar to WAIFs, you want to take care in how you are creating Anonymous Objects, as once they are created, if you continue to reference them in a property, you may have trouble finding them in the future, as there is no way to pull up a list of all Anonymous Objects. 

> Note: The section for [Additional Details on WAIFs](#additional-details-on-waifs) has example verbs that can be used to detect Anonymous Objects referenced in your system.

