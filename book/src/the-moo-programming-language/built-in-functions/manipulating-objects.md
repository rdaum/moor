# Manipulating Objects

Objects are, of course, the main focus of most MOO programming and, largely due to that, there are a lot of built-in functions for manipulating them.

## Fundamental Operations on Objects

### Function: `create`

```
obj create(obj parent [, obj owner] [, int anon-flag] [, list init-args])
obj create(list parents [, obj owner] [, int anon-flag] [, list init-args])
```

Creates and returns a new object whose parent (or parents) is parent (or parents) and whose owner is as described below.

Creates and returns a new object whose parents are parents (or whose parent is parent) and whose owner is as described below. If any of the given parents are not valid, or if the given parent is neither valid nor #-1, then E_INVARG is raised. The given parents objects must be valid and must be usable as a parent (i.e., their `a` or `f` bits must be true) or else the programmer must own parents or be a wizard; otherwise E_PERM is raised. Furthermore, if anon-flag is true then `a` must be true; and, if anon-flag is false or not present, then `f` must be true. Otherwise, E_PERM is raised unless the programmer owns parents or is a wizard. E_PERM is also raised if owner is provided and not the same as the programmer, unless the programmer is a wizard.

After the new object is created, its initialize verb, if any, is called. If init-args were given, they are passed as args to initialize. The new object is assigned the least non-negative object number that has not yet been used for a created object. Note that no object number is ever reused, even if the object with that number is recycled.

> Note: This is not strictly true, especially if you are using ToastCore and the `$recycler`, which is a great idea. If you don't, you end up with extremely high object numbers. However, if you plan on reusing object numbers you need to consider this carefully in your code. You do not want to include object numbers in your code if this is the case, as object numbers could change. Use corified references instead. For example, you can use `@corify #objnum as $my_object` and then be able to reference $my_object in your code. Alternatively you can do `@prop $sysobj.my_object #objnum`. If the object number ever changes, you can change the reference without updating all of your code.)

> Note: $sysobj is typically #0. Though it can technically be changed to something else, there is no reason that the author knows of to break from convention here.

If anon-flag is false or not present, the new object is a permanent object and is assigned the least non-negative object number that has not yet been used for a created object. Note that no object number is ever reused, even if the object with that number is recycled.

If anon-flag is true, the new object is an anonymous object and is not assigned an object number. Anonymous objects are automatically recycled when they are no longer used.

The owner of the new object is either the programmer (if owner is not provided), the new object itself (if owner was given and is invalid, or owner (otherwise).

The other built-in properties of the new object are initialized as follows:

```
name         ""
location     #-1
contents     {}
programmer   0
wizard       0
r            0
w            0
f            0
```

The function `is_player()` returns false for newly created objects.

In addition, the new object inherits all of the other properties on its parents. These properties have the same permission bits as on the parents. If the `c` permissions bit is set, then the owner of the property on the new object is the same as the owner of the new object itself; otherwise, the owner of the property on the new object is the same as that on the parent. The initial value of every inherited property is clear; see the description of the built-in function clear_property() for details.

If the intended owner of the new object has a property named `ownership_quota` and the value of that property is an integer, then create() treats that value as a quota. If the quota is less than or equal to zero, then the quota is considered to be exhausted and create() raises E_QUOTA instead of creating an object. Otherwise, the quota is decremented and stored back into the `ownership_quota` property as a part of the creation of the new object.

> Note: In ToastStunt, this is disabled by default with the "OWNERSHIP_QUOTA" option in options.h

### Function: `owned_objects`

```
list owned_objects(OBJ owner)
```

Returns a list of all objects in the database owned by `owner`. Ownership is defined by the value of .owner on the object.

### Functions: `chparent`, `chparents`

chparent -- Changes the parent of object to be new-parent.

chparents -- Changes the parent of object to be new-parents.

```
none chparent(obj object, obj new-parent)
none chparents(obj object, list new-parents)
```

If object is not valid, or if new-parent is neither valid nor equal to `#-1`, then `E_INVARG` is raised. If the programmer is neither a wizard or the owner of object, or if new-parent is not fertile (i.e., its `f` bit is not set) and the programmer is neither the owner of new-parent nor a wizard, then `E_PERM` is raised. If new-parent is equal to `object` or one of its current ancestors, `E_RECMOVE` is raised. If object or one of its descendants defines a property with the same name as one defined either on new-parent or on one of its ancestors, then `E_INVARG` is raised.

Changing an object's parent can have the effect of removing some properties from and adding some other properties to that object and all of its descendants (i.e., its children and its children's children, etc.). Let common be the nearest ancestor that object and new-parent have in common before the parent of object is changed. Then all properties defined by ancestors of object under common (that is, those ancestors of object that are in turn descendants of common) are removed from object and all of its descendants. All properties defined by new-parent or its ancestors under common are added to object and all of its descendants. As with `create()`, the newly-added properties are given the same permission bits as they have on new-parent, the owner of each added property is either the owner of the object it's added to (if the `c` permissions bit is set) or the owner of that property on new-parent, and the value of each added property is _clear_; see the description of the built-in function `clear_property()` for details. All properties that are not removed or added in the reparenting process are completely unchanged.

If new-parent is equal to `#-1`, then object is given no parent at all; it becomes a new root of the parent/child hierarchy. In this case, all formerly inherited properties on object are simply removed.

If new-parents is equal to {}, then object is given no parent at all; it becomes a new root of the parent/child hierarchy. In this case, all formerly inherited properties on object are simply removed.

> Warning: On the subject of multiple inheritance, the author (Slither) thinks you should completely avoid it. Prefer [composition over inheritance](https://en.wikipedia.org/wiki/Composition_over_inheritance).

### Function: `valid`

```
int valid(obj object)
```

Return a non-zero integer if object is valid and not yet recycled.

Returns a non-zero integer (i.e., a true value) if object is a valid object (one that has been created and not yet recycled) and zero (i.e., a false value) otherwise.

```
valid(#0)    =>   1
valid(#-1)   =>   0
```

### Functions: `parent`, `parents`

parent -- return the parent of object

parents -- return the parents of object

```
obj parent(obj object)
list parents(obj object)
```

### Function: `children`

```
list children(obj object)
```

return a list of the children of object.

### Function: `isa`

```
int isa(OBJ object, OBJ parent)
obj isa(OBJ object, LIST parent list [, INT return_parent])
```

Returns true if object is a descendant of parent, otherwise false.

If a third argument is present and true, the return value will be the first parent that object1 descends from in the `parent list`.

```
isa(#2, $wiz)                           => 1
isa(#2, {$thing, $wiz, $container})     => 1
isa(#2, {$thing, $wiz, $container}, 1)  => #57 (generic wizard)
isa(#2, {$thing, $room, $container}, 1) => #-1
```

### Function: `locate_by_name`

```
list locate_by_name(STR object name)
```

This function searches every object in the database for those containing `object name` in their .name property.

> Warning: Take care when using this when thread mode is active, as this is a threaded function and that means it implicitly suspends. `set_thread_mode(0)` if you want to use this without suspending.

### Function: `locations`

```
list locations(OBJ object [, OBJ stop [, INT is-parent]])
```

Recursively build a list of an object's location, its location's location, and so forth until finally hitting $nothing.

Example:

```
locations(me) => {#20381, #443, #104735}

$string_utils:title_list(locations(me)) => "\"Butterknife Ballet\" Control Room FelElk, the one-person celestial birther \"Butterknife Ballet\", and Uncharted Space: Empty Space"
```

If `stop` is in the locations found, it will stop before there and return the list (exclusive of the stop object).

If the third argument is true, `stop` is assumed to be a PARENT. And if any of your locations are children of that parent, it stops there.

### Function: `occupants`

```
list occupants(LIST objects [, OBJ | LIST parent, INT player flag set?])
```

Iterates through the list of objects and returns those matching a specific set of criteria:

1. If only objects is specified, the occupants function will return a list of objects with the player flag set.

2. If the parent argument is specified, a list of objects descending from parent> will be returned. If parent is a list, object must descend from at least one object in the list.

3. If both parent and player flag set are specified, occupants will check both that an object is descended from parent and also has the player flag set.

### Function: `recycle`

```
none recycle(obj object)
```

destroy object irrevocably.

The given object is destroyed, irrevocably. The programmer must either own object or be a wizard; otherwise, `E_PERM` is raised. If object is not valid, then `E_INVARG` is raised. The children of object are reparented to the parent of object. Before object is recycled, each object in its contents is moved to `#-1` (implying a call to object's `exitfunc` verb, if any) and then object's `recycle` verb, if any, is called with no arguments.

After object is recycled, if the owner of the former object has a property named `ownership_quota` and the value of that property is a integer, then `recycle()` treats that value as a _quota_ and increments it by one, storing the result back into the `ownership_quota` property.

### Function: `recreate`

```
obj recreate(OBJ old, OBJ parent [, OBJ owner])
```

Recreate invalid object old (one that has previously been recycle()ed) as parent, optionally owned by owner.

This has the effect of filling in holes created by recycle() that would normally require renumbering and resetting the maximum object.

The normal rules apply to parent and owner. You either have to own parent, parent must be fertile, or you have to be a wizard. Similarly, to change owner, you should be a wizard. Otherwise it's superfluous.

### Function: `next_recycled_object`

```
obj | int next_recycled_object(OBJ start)
```

Return the lowest invalid object. If start is specified, no object lower than start will be considered. If there are no invalid objects, this function will return 0.

### Function: `recycled_objects`

```
list recycled_objects)
```

Return a list of all invalid objects in the database. An invalid object is one that has been destroyed with the recycle() function.

### Function: `ancestors`

```
list ancestorsOBJ object [, INT full])
```

Return a list of all ancestors of `object` in order ascending up the inheritance hiearchy. If `full` is true, `object` will be included in the list.

### Function: `clear_ancestor_cache`

```
void clear_ancestor_cache()
```

The ancestor cache contains a quick lookup of all of an object's ancestors which aids in expediant property lookups. This is an experimental feature and, as such, you may find that something has gone wrong. If that's that case, this function will completely clear the cache and it will be rebuilt as-needed.

### Function: `descendants`

```
list descendants(OBJ object [, INT full])
```

Return a list of all nested children of object. If full is true, object will be included in the list.

### Function: `object_bytes`

```
int object_bytes(obj object)
```

Returns the number of bytes of the server's memory required to store the given object.

The space calculation includes the space used by the values of all of the objects non-clear properties and by the verbs and properties defined directly on the object.

Raises `E_INVARG` if object is not a valid object and `E_PERM` if the programmer is not a wizard.

### Function: `respond_to`

```
int | list respond_to(OBJ object, STR verb)
```

Returns true if verb is callable on object, taking into account inheritance, wildcards (star verbs), etc. Otherwise, returns false. If the caller is permitted to read the object (because the object's `r' flag is true, or the caller is the owner or a wizard) the true value is a list containing the object number of the object that defines the verb and the full verb name(s).  Otherwise, the numeric value`1' is returned.

### Function: `max_object`

```
obj max_object()
```

Returns the largest object number ever assigned to a created object.

//TODO update for how Toast handles recycled objects if it is different
Note that the object with this number may no longer exist; it may have been recycled. The next object created will be assigned the object number one larger than the value of `max_object()`. The next object getting the number one larger than `max_object()` only applies if you are using built-in functions for creating objects and does not apply if you are using the `$recycler` to create objects.

## Object Movement

### Function: `move`

```
none move(obj what, obj where [, INT position])
```

Changes what's location to be where.

This is a complex process because a number of permissions checks and notifications must be performed. The actual movement takes place as described in the following paragraphs.

what should be a valid object and where should be either a valid object or `#-1` (denoting a location of 'nowhere'); otherwise `E_INVARG` is raised. The programmer must be either the owner of what or a wizard; otherwise, `E_PERM` is raised.

If where is a valid object, then the verb-call

```
where:accept(what)
```

is performed before any movement takes place. If the verb returns a false value and the programmer is not a wizard, then where is considered to have refused entrance to what; `move()` raises `E_NACC`. If where does not define an `accept` verb, then it is treated as if it defined one that always returned false.

If moving what into where would create a loop in the containment hierarchy (i.e., what would contain itself, even indirectly), then `E_RECMOVE` is raised instead.

The `location` property of what is changed to be where, and the `contents` properties of the old and new locations are modified appropriately. Let old-where be the location of what before it was moved. If old-where is a valid object, then the verb-call

```
old-where:exitfunc(what)
```

is performed and its result is ignored; it is not an error if old-where does not define a verb named `exitfunc`. Finally, if where and what are still valid objects, and where is still the location of what, then the verb-call

```
where:enterfunc(what)
```

is performed and its result is ignored; again, it is not an error if where does not define a verb named `enterfunc`.

Passing `position` into move will effectively listinsert() the object into that position in the .contents list.

## Operations on Properties

### Function: `properties`

```
list properties(obj object)
```

Returns a list of the names of the properties defined directly on the given object, not inherited from its parent.

If object is not valid, then `E_INVARG` is raised. If the programmer does not have read permission on object, then `E_PERM` is raised.

### Function: `property_info`

```
list property_info(obj object, str prop-name)
```

Get the owner and permission bits for the property named prop-name on the given object

If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then `E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then `property_info()` raises `E_PERM`.

### Function: `set_property_info`

```
none set_property_info(obj object, str prop-name, list info)
```

Set the owner and permission bits for the property named prop-name on the given object

If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then `E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then `set_property_info()` raises `E_PERM`. Property info has the following form:

```
{owner, perms [, new-name]}
```

where owner is an object, perms is a string containing only characters from the set `r`, `w`, and `c`, and new-name is a string; new-name is never part of the value returned by `property_info()`, but it may optionally be given as part of the value provided to `set_property_info()`. This list is the kind of value returned by property_info() and expected as the third argument to `set_property_info()`; the latter function raises `E_INVARG` if owner is not valid, if perms contains any illegal characters, or, when new-name is given, if prop-name is not defined directly on object or new-name names an existing property defined on object or any of its ancestors or descendants.

### Function: `add_property`

```
none add_property(obj object, str prop-name, value, list info)
```

Defines a new property on the given object

The property is inherited by all of its descendants; the property is named prop-name, its initial value is value, and its owner and initial permission bits are given by info in the same format as is returned by `property_info()`, described above.

If object is not valid or info does not specify a valid owner and well-formed permission bits or object or its ancestors or descendants already defines a property named prop-name, then `E_INVARG` is raised. If the programmer does not have write permission on object or if the owner specified by info is not the programmer and the programmer is not a wizard, then `E_PERM` is raised.

### Function: `delete_property`

```
none delete_property(obj object, str prop-name)
```

Removes the property named prop-name from the given object and all of its descendants.

If object is not valid, then `E_INVARG` is raised. If the programmer does not have write permission on object, then `E_PERM` is raised. If object does not directly define a property named prop-name (as opposed to inheriting one from its parent), then `E_PROPNF` is raised.

### Function: `is_clear_property`

```
int is_clear_property(obj object, str prop-name) ##### Function: `clear_property`
```

Test the specified property for clear

clear_property -- Set the specified property to clear

none `clear_property` (obj object, str prop-name)

These two functions test for clear and set to clear, respectively, the property named prop-name on the given object. If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then `E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then `is_clear_property()` (`clear_property()`) raises `E_PERM`.

If a property is clear, then when the value of that property is queried the value of the parent's property of the same name is returned. If the parent's property is clear, then the parent's parent's value is examined, and so on. If object is the definer of the property prop-name, as opposed to an inheritor of the property, then `clear_property()` raises `E_INVARG`.

## Operations on Verbs

### Function: `verbs`

```
list verbs(obj object)
```

Returns a list of the names of the verbs defined directly on the given object, not inherited from its parent

If object is not valid, then `E_INVARG` is raised. If the programmer does not have read permission on object, then `E_PERM` is raised.

Most of the remaining operations on verbs accept a string containing the verb's name to identify the verb in question. Because verbs can have multiple names and because an object can have multiple verbs with the same name, this practice can lead to difficulties. To most unambiguously refer to a particular verb, one can instead use a positive integer, the index of the verb in the list returned by `verbs()`, described above.

For example, suppose that `verbs(#34)` returns this list:

```
{"foo", "bar", "baz", "foo"}
```

Object `#34` has two verbs named `foo` defined on it (this may not be an error, if the two verbs have different command syntaxes). To refer unambiguously to the first one in the list, one uses the integer 1; to refer to the other one, one uses 4.

In the function descriptions below, an argument named verb-desc is either a string containing the name of a verb or else a positive integer giving the index of that verb in its defining object's `verbs()` list.
For historical reasons, there is also a second, inferior mechanism for referring to verbs with numbers, but its use is strongly discouraged. If the property `$server_options.support_numeric_verbname_strings` exists with a true value, then functions on verbs will also accept a numeric string (e.g., `"4"`) as a verb descriptor. The decimal integer in the string works more-or-less like the positive integers described above, but with two significant differences:

The numeric string is a _zero-based_ index into `verbs()`; that is, in the string case, you would use the number one less than what you would use in the positive integer case.

When there exists a verb whose actual name looks like a decimal integer, this numeric-string notation is ambiguous; the server will in all cases assume that the reference is to the first verb in the list for which the given string could be a name, either in the normal sense or as a numeric index.

Clearly, this older mechanism is more difficult and risky to use; new code should only be written to use the current mechanism, and old code using numeric strings should be modified not to do so.

### Function: `verb_info`

```
list verb_info(obj object, str|int verb-desc)
```

Get the owner, permission bits, and name(s) for the verb as specified by verb-desc on the given object

### Function: `set_verb_info`

set_verb_info -- Set the owner, permissions bits, and names(s) for the verb as verb-desc on the given object

none `set_verb_info` (obj object, str|int verb-desc, list info)

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read (write) permission on the verb in question, then `verb_info()` (`set_verb_info()`) raises `E_PERM`.

Verb info has the following form:

```
{owner, perms, names}
```

where owner is an object, perms is a string containing only characters from the set `r`, `w`, `x`, and `d`, and names is a string. This is the kind of value returned by `verb_info()` and expected as the third argument to `set_verb_info()`. `set_verb_info()` raises `E_INVARG` if owner is not valid, if perms contains any illegal characters, or if names is the empty string or consists entirely of spaces; it raises `E_PERM` if owner is not the programmer and the programmer is not a wizard.

### Function: `verb_args`

```
list verb_args(obj object, str|int verb-desc)
```

Get the direct-object, preposition, and indirect-object specifications for the verb as specified by verb-desc on the given object.

### Function: `set_verb_args`

verb_args -- set the direct-object, preposition, and indirect-object specifications for the verb as specified by verb-desc on the given object.

none `set_verb_args` (obj object, str|int verb-desc, list args)

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read (write) permission on the verb in question, then the function raises `E_PERM`.

Verb args specifications have the following form:

```
{dobj, prep, iobj}
```

where dobj and iobj are strings drawn from the set `"this"`, `"none"`, and `"any"`, and prep is a string that is either `"none"`, `"any"`, or one of the prepositional phrases listed much earlier in the description of verbs in the first chapter. This is the kind of value returned by `verb_args()` and expected as the third argument to `set_verb_args()`. Note that for `set_verb_args()`, prep must be only one of the prepositional phrases, not (as is shown in that table) a set of such phrases separated by `/` characters. `set_verb_args` raises `E_INVARG` if any of the dobj, prep, or iobj strings is illegal.

```
verb_args($container, "take")
                    =>   {"any", "out of/from inside/from", "this"}
set_verb_args($container, "take", {"any", "from", "this"})
```

### Function: `add_verb`

```
none add_verb(obj object, list info, list args)
```

Defines a new verb on the given object

The new verb's owner, permission bits and name(s) are given by info in the same format as is returned by `verb_info()`, described above. The new verb's direct-object, preposition, and indirect-object specifications are given by args in the same format as is returned by `verb_args`, described above. The new verb initially has the empty program associated with it; this program does nothing but return an unspecified value.

If object is not valid, or info does not specify a valid owner and well-formed permission bits and verb names, or args is not a legitimate syntax specification, then `E_INVARG` is raised. If the programmer does not have write permission on object or if the owner specified by info is not the programmer and the programmer is not a wizard, then `E_PERM` is raised.

### Function: `delete_verb`

```
none delete_verb(obj object, str|int verb-desc)
```

Removes the verb as specified by verb-desc from the given object

If object is not valid, then `E_INVARG` is raised. If the programmer does not have write permission on object, then `E_PERM` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised.

### Function: `verb_code`

```
list verb_code(obj object, str|int verb-desc [, fully-paren [, indent]])
```

Get the MOO-code program associated with the verb as specified by verb-desc on object

### Function: `set_verb_code`

```
list set_verb_code(obj object, str|int verb-desc, list code)
```

Set the MOO-code program associated with the verb as specified by verb-desc on object

The program is represented as a list of strings, one for each line of the program; this is the kind of value returned by `verb_code()` and expected as the third argument to `set_verb_code()`. For `verb_code()`, the expressions in the returned code are usually written with the minimum-necessary parenthesization; if full-paren is true, then all expressions are fully parenthesized.

Also for `verb_code()`, the lines in the returned code are usually not indented at all; if indent is true, each line is indented to better show the nesting of statements.

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read (write) permission on the verb in question, then `verb_code()` (`set_verb_code()`) raises `E_PERM`. If the programmer is not, in fact. a programmer, then `E_PERM` is raised.

For `set_verb_code()`, the result is a list of strings, the error messages generated by the MOO-code compiler during processing of code. If the list is non-empty, then `set_verb_code()` did not install code; the program associated with the verb in question is unchanged.

### Function: `disassemble`

```
list disassemble(obj object, str|int verb-desc)
```

Returns a (longish) list of strings giving a listing of the server's internal "compiled" form of the verb as specified by verb-desc on object

This format is not documented and may indeed change from release to release, but some programmers may nonetheless find the output of `disassemble()` interesting to peruse as a way to gain a deeper appreciation of how the server works.

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read permission on the verb in question, then `disassemble()` raises `E_PERM`.

## Operations on WAIFs

### Function: `new_waif`

```
waif new_waif)
```

The `new_waif()` builtin creates a new WAIF whose class is the calling object and whose owner is the perms of the calling verb.

This wizardly version causes it to be owned by the caller of the verb.

### Function: `waif_stats`

```
map waif_stats()
```

Returns a MAP of statistics about instantiated waifs.

Each waif class will be a key in the MAP and its value will be the number of waifs of that class currently instantiated. Additionally, there is a `total' key that will return the total number of instantiated waifs, and a`pending_recycle' key that will return the number of waifs that have been destroyed and are awaiting the call of their :recycle verb.

## Operations on Player Objects

### Function: `players`

```
list players()
```

Returns a list of the object numbers of all player objects in the database

### Function: `is_player`

```
int is_player(obj object)
```

Returns a true value if the given object is a player object and a false value otherwise.

If object is not valid, `E_INVARG` is raised.

### Function: `set_player_flag`

```
none set_player_flag(obj object, value)
```

Confers or removes the "player object" status of the given object, depending upon the truth value of value

If object is not valid, `E_INVARG` is raised. If the programmer is not a wizard, then `E_PERM` is raised.

If value is true, then object gains (or keeps) "player object" status: it will be an element of the list returned by `players()`, the expression `is_player(object)` will return true, and the server will treat a call to `$do_login_command()` that returns object as logging in the current connection.

If value is false, the object loses (or continues to lack) "player object" status: it will not be an element of the list returned by `players()`, the expression `is_player(object)` will return false, and users cannot connect to object by name when they log into the server. In addition, if a user is connected to object at the time that it loses "player object" status, then that connection is immediately broken, just as if `boot_player(object)` had been called (see the description of `boot_player()` below).
