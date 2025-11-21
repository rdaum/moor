# Manipulating Objects

Objects are, of course, the main focus of most MOO programming and, largely due to that, there are a lot of built-in
functions for manipulating them.

## Fundamental Operations on Objects

### `create`

```
obj create(obj parent [, obj owner] [, int obj_type] [, list init-args])
```

Creates and returns a new object whose parent is parent and whose owner is as described below. If the given parent is
neither valid nor #-1, then E_INVARG is raised. The parent object must be valid and must be usable as a parent (i.e.,
its `f` bit must be true) or else the programmer must own parent or be a wizard; otherwise E_PERM is raised. If the `f`
bit is not present, E_PERM is raised unless the programmer owns parent or is a wizard.

The `obj_type` argument controls which type of object to create:

- `0` (or `false`) = numbered objects like `#123` (default)
- `1` (or `true`) = anonymous objects (requires `anonymous_objects` feature enabled)
- `2` = UUID objects like `#048D05-1234567890`

If `obj_type` is `1` but the `anonymous_objects` feature is not enabled, `E_INVARG` is raised.

E_PERM is also raised if owner is provided and not the same as the programmer, unless the programmer is a wizard.

After the new object is created, its initialize verb, if any, is called. If init-args were given, they are passed as
args to initialize. For numbered objects, the new object is assigned the least non-negative object number that has not
yet been used. For UUID objects, a unique UUID identifier is generated. For anonymous objects, an opaque reference is
created that cannot be typed directly in code.

Note that numbered object numbers are never reused, even if the object with that number is recycled. UUID and anonymous
objects do not have this limitation.

> Note: $sysobj is typically #0. Though it can technically be changed to something else, there is no reason that the
> author knows of to break from convention here.

The owner of the new object is either the programmer (if owner is not provided), the new object itself (if owner was
given and is invalid, or owner (otherwise).

### `create_at`

```
obj create_at(obj id, obj parent [, obj owner] [, list init-args])
```

Creates and returns a new object at the specified object ID. This function is wizard-only and allows creating objects
with specific object numbers rather than using the next available number.

If the specified object ID already exists, E_PERM is raised. The parent, owner, and init-args parameters work exactly
the same as in create(). After the new object is created, its initialize verb, if any, is called with the provided
init-args.

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

In addition, the new object inherits all of the other properties on its parents. These properties have the same
permission bits as on the parents. If the `c` permissions bit is set, then the owner of the property on the new object
is the same as the owner of the new object itself; otherwise, the owner of the property on the new object is the same as
that on the parent. The initial value of every inherited property is clear; see the description of the built-in function
clear_property() for details.

If the intended owner of the new object has a property named `ownership_quota` and the value of that property is an
integer, then create() treats that value as a quota. If the quota is less than or equal to zero, then the quota is
considered to be exhausted and create() raises E_QUOTA instead of creating an object. Otherwise, the quota is
decremented and stored back into the `ownership_quota` property as a part of the creation of the new object.

### `owned_objects`

```
list owned_objects(OBJ owner)
```

Returns a list of all objects in the database owned by `owner`. Ownership is defined by the value of .owner on the
object.

### `chparent`

chparent -- Changes the parent of object to be new-parent.

```
none chparent(obj object, obj new-parent)
```

If object is not valid, or if new-parent is neither valid nor equal to `#-1`, then `E_INVARG` is raised. If the
programmer is neither a wizard or the owner of object, or if new-parent is not fertile (i.e., its `f` bit is not set)
and the programmer is neither the owner of new-parent nor a wizard, then `E_PERM` is raised. If new-parent is equal to
`object` or one of its current ancestors, `E_RECMOVE` is raised. If object or one of its descendants defines a property
with the same name as one defined either on new-parent or on one of its ancestors, then `E_INVARG` is raised.

Changing an object's parent can have the effect of removing some properties from and adding some other properties to
that object and all of its descendants (i.e., its children and its children's children, etc.). Let common be the nearest
ancestor that object and new-parent have in common before the parent of object is changed. Then all properties defined
by ancestors of object under common (that is, those ancestors of object that are in turn descendants of common) are
removed from object and all of its descendants. All properties defined by new-parent or its ancestors under common are
added to object and all of its descendants. As with `create()`, the newly-added properties are given the same permission
bits as they have on new-parent, the owner of each added property is either the owner of the object it's added to (if
the `c` permissions bit is set) or the owner of that property on new-parent, and the value of each added property is
_clear_; see the description of the built-in function `clear_property()` for details. All properties that are not
removed or added in the reparenting process are completely unchanged.

If new-parent is equal to `#-1`, then object is given no parent at all; it becomes a new root of the parent/child
hierarchy. In this case, all formerly inherited properties on object are simply removed.

### `valid`

```
int valid(obj object)
```

Return a non-zero integer if object is valid and not yet recycled.

Returns a non-zero integer (i.e., a true value) if object is a valid object (one that has been created and not yet
recycled) and zero (i.e., a false value) otherwise.

```
valid(#0)    =>   1
valid(#-1)   =>   0
```

### `is_anonymous`

```
int is_anonymous(obj object)
```

Returns a non-zero integer (i.e., a true value) if object is an anonymous object and zero (i.e., a false value)
otherwise.

Since anonymous objects have `typeof(obj) == OBJ` (same as regular objects), this is the only way to distinguish them
programmatically.

```
let regular_obj = create($thing);         // Regular numbered/UUID object  
let anon_obj = create($thing, player, 1); // Anonymous object

is_anonymous(regular_obj);  =>  0   // false - not anonymous
is_anonymous(anon_obj);     =>  1   // true - is anonymous
typeof(anon_obj);          =>  1   // OBJ - same as regular objects!
```

If object is not a valid object, `E_INVARG` is raised.

> **Porting from ToastStunt**: In ToastStunt, anonymous objects had `typeof(anon_obj) == ANON`. In mooR,
> anonymous objects have `typeof(anon_obj) == OBJ` and you must use `is_anonymous()` to detect them.

### `is_uuobjid`

```
int is_uuobjid(obj object)
```

Returns a non-zero integer (i.e., a true value) if object is a UUID object (like `#048D05-1234567890`) and zero (i.e., a
false value) otherwise.

Since UUID objects have `typeof(obj) == OBJ` (same as numbered and anonymous objects), this is the way to distinguish
them programmatically.

```
let numbered_obj = create($thing);        // Regular numbered object
let uuid_obj = create($thing, player, 0); // UUID object (if enabled)

is_uuobjid(numbered_obj);  =>  0   // false - not UUID
is_uuobjid(uuid_obj);      =>  1   // true - is UUID
typeof(uuid_obj);         =>  1   // OBJ - same as other objects!
```

If object is not a valid object, `E_INVARG` is raised.

> **Feature requirement**: UUID objects require the `use_uuobjids` feature flag to be enabled by the server
> administrator.

### Functions: `parent`

parent -- return the parent of object

```
obj parent(obj object)
```

### `children`

```
list children(obj object)
```

return a list of the children of object.

### `isa`

```
int isa(OBJ object, OBJ parent)
obj isa(OBJ object, LIST parent list [, INT return_parent])
```

Returns true if object is a descendant of parent, otherwise false.

If a third argument is present and true, the return value will be the first parent that object1 descends from in the
`parent list`.

```
isa(#2, $wiz)                           => 1
isa(#2, {$thing, $wiz, $container})     => 1
isa(#2, {$thing, $wiz, $container}, 1)  => #57 (generic wizard)
isa(#2, {$thing, $room, $container}, 1) => #-1
```

### `locate_by_name`

```
obj locate_by_name([obj object,] str name [, INT with_key])
```

`object.name` is a string and may optionally contain a key field. The key field is separated from the name by "  [", and
its value is delimited by the first space or end of string.

This function is primarily designed to return the best match to `name` of the children of `object`. This function is
used by the MOO to look for objects referenced via the input functions. This mimics the behavior of the lambda MOO. If
`name` is a valid object number, then the object representing that number is returned. If not, the name is tested
against the objects in `object.contents`.

If `with_key` is specified and true, and a key is supplied in the name, the key is tested against the object key
specified in the objects name. If the object has a key and the key in the name doesn't match, the object is rejected
from the search.

```
obj:locate_by_name("bar")        =>   #0 (first match)
obj:locate_by_name("foo [3]")    =>   matches object #0 with key "3" only.
obj:locate_by_name("foo [3]", 1) =>   same as above
obj:locate_by_name("foo [3]", 0) =>   would return the first "foo" object, ignoring key check
```

### `recycle`

```
none recycle(obj object)
```

destroy object irrevocably.

The given object is destroyed, irrevocably. The programmer must either own object or be a wizard; otherwise, `E_PERM` is
raised. If object is not valid, then `E_INVARG` is raised. The children of object are reparented to the parent of
object. Before object is recycled, each object in its contents is moved to `#-1` (implying a call to object's `exitfunc`
verb, if any) and then object's `recycle` verb, if any, is called with no arguments.

After object is recycled, if the owner of the former object has a property named `ownership_quota` and the value of that
property is a integer, then `recycle()` treats that value as a _quota_ and increments it by one, storing the result back
into the `ownership_quota` property.

### `recreate`

```
obj recreate(OBJ old, OBJ parent [, OBJ owner])
```

Recreate invalid object old (one that has previously been recycle()ed) as parent, optionally owned by owner.

This has the effect of filling in holes created by recycle() that would normally require renumbering and resetting the
maximum object.

The normal rules apply to parent and owner. You either have to own parent, parent must be fertile, or you have to be a
wizard. Similarly, to change owner, you should be a wizard. Otherwise it's superfluous.

### `ancestors`

```
list ancestorsOBJ object [, INT full])
```

Return a list of all ancestors of `object` in order ascending up the inheritance hiearchy. If `full` is true, `object`
will be included in the list.

### `clear_ancestor_cache`

```
void clear_ancestor_cache()
```

The ancestor cache contains a quick lookup of all of an object's ancestors which aids in expediant property lookups.
This is an experimental feature and, as such, you may find that something has gone wrong. If that's that case, this
function will completely clear the cache and it will be rebuilt as-needed.

### `descendants`

```
list descendants(OBJ object [, INT full])
```

Return a list of all nested children of object. If full is true, object will be included in the list.

### `object_bytes`

```
int object_bytes(obj object)
```

Returns the number of bytes of the server's memory required to store the given object.

The space calculation includes the space used by the values of all of the objects non-clear properties and by the verbs
and properties defined directly on the object.

Raises `E_INVARG` if object is not a valid object and `E_PERM` if the programmer is not a wizard.

### `respond_to`

```
int | list respond_to(OBJ object, STR verb)
```

Returns true if verb is callable on object, taking into account inheritance, wildcards (star verbs), etc. Otherwise,
returns false. If the caller is permitted to read the object (because the object's
`r' flag is true, or the caller is the owner or a wizard) the true value is a list containing the object number of the object that defines the verb and the full verb name(s).  Otherwise, the numeric value`
1' is returned.

### `max_object`

```
obj max_object()
```

Returns the largest object number ever assigned to a created object.

//TODO update for how Toast handles recycled objects if it is different
Note that the object with this number may no longer exist; it may have been recycled. The next object created will be
assigned the object number one larger than the value of `max_object()`. The next object getting the number one larger
than `max_object()` only applies if you are using built-in functions for creating objects and does not apply if you are
using the `$recycler` to create objects.

### `renumber`

```
obj renumber(obj old_obj [, obj target])
```

Renumbers an object to a new object ID. This moves the object itself and everything defined on it, but does not update
references to the object stored elsewhere.

If `target` is provided, the object `old_obj` is renumbered to have the ID `target`. The target object ID must not
already exist, or `E_INVARG` is raised.

If `target` is not provided (auto-selection), the server chooses an appropriate new object ID:

- For numbered objects: Scans from `#0` to `old_obj - 1` to find the first available slot (following LambdaMOO
  semantics)
- For UUID objects: Scans backwards from `max_object()` to `#0` to find the first available numbered slot; if none
  found,
  uses `max_object() + 1`

**What renumber updates automatically:**

- The object's properties, verbs, and other content move with it to the new ID
- Structural database relationships where other objects point to this object:
    - Parent-child relationships (objects that have this object as their parent)
    - Location-contents relationships (objects that are located in this object)
    - Owner relationships (objects that are owned by this object)

**What renumber does NOT update:**

- References to this object stored in property values on other objects
- References to this object in verb code anywhere in the database
- Any other stored references to the old object ID

After renumbering, the old object ID becomes invalid and the object is only accessible by its new ID. Any remaining
references to the old ID will need to be manually updated by the programmer.

Cross-type renumbering restrictions:

- `renumber(uuid, uuid)` - Not allowed, raises `E_INVARG`
- `renumber(obj, uuid)` - Not allowed, raises `E_INVARG`
- `renumber(uuid)` - Allowed, converts UUID to numbered object with auto-selection
- `renumber(uuid, obj)` - Allowed, converts UUID to specific numbered object
- `renumber(obj)` - Allowed, finds new numbered slot for numbered object
- `renumber(obj, obj)` - Allowed, moves numbered object to specific numbered slot

The programmer must own the object being renumbered or be a wizard, otherwise `E_PERM` is raised. If `old_obj` is not
valid, `E_INVARG` is raised.

After renumbering, `max_object()` is updated if the new object ID is higher than the current maximum (for numbered
objects only).

```
renumber(#123)                    =>   #45 (found first available slot)
renumber(#048D05-1234567890)      =>   #241 (UUID converted to numbered)
renumber(#123, #500)              =>   #500 (explicit target)
renumber(#048D05-1234567890, #99) =>   #99 (UUID converted to specific numbered)
```

## Object Movement

### `move`

```
none move(obj what, obj where [, INT position])
```

Changes what's location to be where.

This is a complex process because a number of permissions checks and notifications must be performed. The actual
movement takes place as described in the following paragraphs.

what should be a valid object and where should be either a valid object or `#-1` (denoting a location of 'nowhere');
otherwise `E_INVARG` is raised. The programmer must be either the owner of what or a wizard; otherwise, `E_PERM` is
raised.

If where is a valid object, then the verb-call

```
where:accept(what)
```

is performed before any movement takes place. If the verb returns a false value and the programmer is not a wizard, then
where is considered to have refused entrance to what; `move()` raises `E_NACC`. If where does not define an `accept`
verb, then it is treated as if it defined one that always returned false.

If moving what into where would create a loop in the containment hierarchy (i.e., what would contain itself, even
indirectly), then `E_RECMOVE` is raised instead.

The `location` property of what is changed to be where, and the `contents` properties of the old and new locations are
modified appropriately. Let old-where be the location of what before it was moved. If old-where is a valid object, then
the verb-call

```
old-where:exitfunc(what)
```

is performed and its result is ignored; it is not an error if old-where does not define a verb named `exitfunc`.
Finally, if where and what are still valid objects, and where is still the location of what, then the verb-call

```
where:enterfunc(what)
```

is performed and its result is ignored; again, it is not an error if where does not define a verb named `enterfunc`.

Passing `position` into move will effectively listinsert() the object into that position in the .contents list.

## Operations on Properties

### `properties`

```
list properties(obj object)
```

Returns a list of the names of the properties defined directly on the given object, not inherited from its parent.

If object is not valid, then `E_INVARG` is raised. If the programmer does not have read permission on object, then
`E_PERM` is raised.

### `property_info`

```
list property_info(obj object, str prop-name)
```

Get the owner and permission bits for the property named prop-name on the given object

If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then
`E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then
`property_info()` raises `E_PERM`.

### `set_property_info`

```
none set_property_info(obj object, str prop-name, list info)
```

Set the owner and permission bits for the property named prop-name on the given object

If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then
`E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then
`set_property_info()` raises `E_PERM`. Property info has the following form:

```
{owner, perms [, new-name]}
```

where owner is an object, perms is a string containing only characters from the set `r`, `w`, and `c`, and new-name is a
string; new-name is never part of the value returned by `property_info()`, but it may optionally be given as part of the
value provided to `set_property_info()`. This list is the kind of value returned by property_info() and expected as the
third argument to `set_property_info()`; the latter function raises `E_INVARG` if owner is not valid, if perms contains
any illegal characters, or, when new-name is given, if prop-name is not defined directly on object or new-name names an
existing property defined on object or any of its ancestors or descendants.

### `add_property`

```
none add_property(obj object, str prop-name, value, list info)
```

Defines a new property on the given object

The property is inherited by all of its descendants; the property is named prop-name, its initial value is value, and
its owner and initial permission bits are given by info in the same format as is returned by `property_info()`,
described above. If object is not valid or info does not have the correct format, then `E_INVARG` is raised. If the
programmer does not have write permission on object, if an ancestor or descendant of object already defines a property
named prop-name, or if the owner specified by info is not valid, then `E_PERM` is raised.

### `delete_property`

```
none delete_property(obj object, str prop-name)
```

Removes the property named prop-name from the given object.

If object is not valid, then `E_INVARG` is raised. If the programmer does not have write permission on object, then
`E_PERM` is raised. If object does not directly define a property named prop-name (as opposed to inheriting one from its
parent), then `E_PROPNF` is raised.

### `clear_property`

```
none clear_property(obj object, str prop-name)
```

Sets the value of the property named prop-name on the given object to 'clear.'

If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then
`E_PROPNF` is raised. If the programmer does not have write permission on the property in question, then
`clear_property()` raises `E_PERM`. clear_property() sets the value of the property to a special value called clear. In
particular, when the property is next read, the value returned will be the value of that property on the parent object.
If the parent object does not have a value for that property, then the grandparent is consulted, and so on. In the
unusual case that there is not even a clear value all the way up to the root object, the value 0 is returned. This
inheritance behavior is the same as if the property had never been assigned a value on the object in question. If the
property had never been defined on the object in question, an attempt to read it would result in `E_PROPNF`, but if it
is defined but clear, the inheritance behavior described here applies.

### `has_property`

```
int has_property(OBJ object, STR name [, INT return_propdef])
```

Return whether or not the given object has the named property (considering inheritance). When given the optional third
argument with a true value, return the object with the property defined on it (taking into account inheritance).

## Operations on Verbs

### `verbs`

```
list verbs(obj object)
```

Returns a list of the names of the verbs defined directly on the given object, not inherited from its parent

If object is not valid, then `E_INVARG` is raised. If the programmer does not have read permission on object, then most
of the remainder of this section on verb-manipulating functions applies:

For the functions described in the next section, if object is not valid, then `E_INVARG` is raised. If object does not
define a verb named verb-name, then `E_VERBNF` is raised. If the programmer does not have read permission on object,
then `E_PERM` is raised.

### `verb_info`

```
list verb_info(obj object, str verb-name)
```

Returns a list of three items: the owner of the named verb, a string containing the permission bits for the named verb,
and a string containing the names that the named verb can go by.

### `set_verb_info`

```
none set_verb_info(obj object, str verb-name, list info)
```

Changes the owner, permission bits, and/or names for the named verb.

Info must be a list of three items as would be returned by `verb_info()`, described above.

### `verb_args`

```
list verb_args(obj object, str verb-name)
```

Return information about the names and types of the arguments to the named verb

The return value is a list of three items:

- a string containing the direct-object specification for this verb
- a string containing the preposition specification for this verb
- a string containing the indirect-object specification for this verb

The specifications are strings like those allowed in the grammar for verb declarations; see The MOO Programming Language
for details.

### `set_verb_args`

```
none set_verb_args(obj object, str verb-name, list args)
```

Change the specifications of the arguments for the named verb

Args must be a list of three strings as would be returned by `verb_args()`, described above.

### `add_verb`

```
none add_verb(obj object, list info, list args)
```

Defines a new verb on the given object.

The new verb's owner, permission bits and names are given by info in the same format as is returned by `verb_info()`,
described above. The new verb's direct-, preposition, and indirect-object specifications are given by args in the same
format as is returned by `verb_args()`, described above. The new verb initially has the empty program associated with
it; this program does nothing but return an unspecified value.

If object is not valid, or info does not have the correct format, then `E_INVARG` is raised. If the programmer does not
have write permission on object, if the owner specified by info is not valid, or if the programmer is not a wizard and
the owner specified by info is not the same as the programmer, then `E_PERM` is raised.

### `delete_verb`

```
none delete_verb(obj object, str verb-name)
```

Removes the named verb from the given object.

If object is not valid, then `E_INVARG` is raised. If the programmer does not have write permission on object, then
`E_PERM` is raised. If object does not define a verb named verb-name, then `E_VERBNF` is raised.

### `verb_code`

```
list verb_code(obj object, str verb-name [, fully-paren [, indent]])
```

Returns a list of strings giving the MOO-language statements comprising the program for the named verb.

This program is the same collection of statements that would be entered in the editor to change the program for the
named verb. The `fully-paren` controls whether or not the program is printed with full parentheses around all
expressions; if `fully-paren` is true, then all expressions are fully parenthesized, if false they are printed in the
customary MOO syntax, and if `fully-paren` is not provided it defaults to false. The `indent` argument controls whether
statements are indented; if `indent` is not provided, it defaults to true.

Note that the list returned by verb_code() is not necessarily the same as the one used in a previous call to
`set_verb_code()` (described below) to set the program for this verb. The list returned by `verb_code()` is always a
canonicalized version of the program: white-space is standardized, comments are removed, etc.

### `set_verb_code`

```
list set_verb_code(obj object, str verb-name, list program)
```

Sets the MOO-language program for the named verb to the given list of statements.

The result is a list of strings, the error messages generated by the MOO-language compiler during processing of program.
If the result is non-empty, then the operation was not successful and the program for the named verb is unchanged;
otherwise, the operation was successful and the program for the named verb is now program.

The elements of program should be strings containing MOO statements; if any of the elements is not a string, then
`E_INVARG` is raised. The program need not be syntactically correct MOO; if it is not, then the operation fails and a
non-empty list of compiler error messages is returned. The program may be syntactically correct but suffer from one or
more MOO compile-time semantic errors (e.g., syntax that would exceed certain built-in MOO limits); if so, the operation
fails and a non-empty list of compiler error messages is returned.

If object is not valid, then `E_INVARG` is raised. If the programmer does not have write permission on object, then
`E_PERM` is raised. If object does not define a verb named verb-name, then `E_VERBNF` is raised.

### `eval`

```
list eval(str string)
```

The MOO-language expression (or statement) given in string is compiled and evaluated.

The result is a list of two values: a flag indicating whether or not the operation was successful and a value whose
interpretation depends upon the success flag. If the flag is true, then the operation was successful and the value is
the result of the evaluation. If the flag is false, the operation failed and the value is a list of strings giving
error messages generated by the compiler.

The `string` is compiled as if it were written on a single line of a verb; in particular, a return statement in string
can return a value from the current verb. The expression (or statement) operates in the context of the current verb
call and has access to the same built-in variables and any verb-local variables.

This operation raises `E_INVARG` if the programmer is not, in fact, a programmer.

## Object Owners and Wizards

### `players`

```
list players()
```

Returns a list of the object numbers of all player objects in the database

### `objects`

```
list objects()
```

Returns a list of all valid object numbers in the database. This includes all objects that have been created and not yet
recycled, regardless of whether they are player objects, numbered objects, UUID objects, or anonymous objects.

This function is wizard-only. If the programmer is not a wizard, `E_PERM` is raised.

> **Note**: This is a mooR-specific extension not present in LambdaMOO or ToastStunt. It provides a way to enumerate all
> valid objects in the database for administrative purposes.

```
objects()    =>   {#0, #1, #2, #5, #048D05-1234567890, ...}
```

> **Warning**: In large databases, this function may be slow and return a very large list. Use with caution in
> production environments.

### `is_player`

```
int is_player(obj object)
```

Returns a true value if the given object is a player object and a false value otherwise.

If object is not valid, `E_INVARG` is raised.

### `set_player_flag`

```
none set_player_flag(obj object, value)
```

Confers or removes the "player object" status of the given object, depending upon the truth value of value

If object is not valid, `E_INVARG` is raised. If the programmer is not a wizard, then `E_PERM` is raised.

If value is true, then object gains (or keeps) "player object" status: it will be an element of the list returned by
`players()`, the expression `is_player(object)` will return true, and the server will treat a call to
`$do_login_command()` that returns object as logging in the current connection.

If value is false, the object loses (or continues to lack) "player object" status: it will not be an element of the list
returned by `players()`, the expression `is_player(object)` will return false, and users cannot connect to object by
name when they log into the server. In addition, if a user is connected to object at the time that it loses "player
object" status, then that connection is immediately broken, just as if `boot_player(object)` had been called (see the
description of `boot_player()` below).

---

## Command Parsing and Dispatching

These functions allow programmers to implement custom command parsing, ambiguity resolution, and verb dispatching.
They provide low-level access to the command parser components, enabling more sophisticated command handling systems
than the naive builtin one, while still using its components.

### `parse_command`

```
map parse_command(str command, list environment [, bool complex])
```

Parses a command string into its components (verb, objects, preposition) and returns a map with the results.

**Arguments:**

- `command`: The command string to parse (e.g., `"get lamp"`, `"put lamp on table"`)
- `environment`: A list of objects to search for object name matching. Each entry can be:
    - A simple object (e.g., `player`)
    - A list with format `{object, "name1", "name2", ...}` to provide custom aliases
- `complex`: (Optional, default false) When true, enables fuzzy matching and ordinal support (e.g., "first lamp", "
  second bottle")

**Returns:** A map with the following keys:

| Key              | Type          | Description                                                           |
|------------------|---------------|-----------------------------------------------------------------------|
| `verb`           | symbol/string | The parsed verb (first word of command)                               |
| `argstr`         | string        | Everything after the first word                                       |
| `args`           | list          | List of individual argument strings                                   |
| `dobjstr`        | string        | Direct object string that was matched (empty if none)                 |
| `dobj`           | object        | Direct object found, or `#-1` if none                                 |
| `ambiguous_dobj` | list          | List of objects if dobj was ambiguous, empty list otherwise           |
| `prepstr`        | string        | Preposition string found (empty if none)                              |
| `prep`           | int           | Preposition code: `-2`=any, `-1`=none, `0`-`14`=specific prepositions |
| `iobjstr`        | string        | Indirect object string (empty if none)                                |
| `iobj`           | object        | Indirect object found, or `#-1` if none                               |
| `ambiguous_iobj` | list          | List of objects if iobj was ambiguous, empty list otherwise           |

**Notes:**

- Object matching searches through the provided environment list
- Multiple matching objects are reported in `ambiguous_dobj` or `ambiguous_iobj`
- The `complex` parameter enables the enhanced matching system with ordinals and fuzzy matching
- This is a wizard-only function when called directly; see [`find_command_verb`](#find_command_verb) for finding actual
  verbs

**Example:**

```moo
cmd = "get lamp";
env = {player, player.location};
pc = parse_command(cmd, env);
// Returns:
// ["verb" -> "get", "argstr" -> "lamp", "args" -> {"lamp"},
//  "dobjstr" -> "lamp", "dobj" -> #123, "iobj" -> #-1, "prep" -> -1, ...]
```

### `find_command_verb`

```
list find_command_verb(map parsed_command_spec, list command_environment)
```

Searches for command verbs that match a parsed command specification across a set of target objects.

**Arguments:**

- `parsed_command_spec`: A map returned from [`parse_command()`](#parse_command) containing the parsed command
  components
- `command_environment`: A list of objects to search for verbs (typically `{player, player.location}`)

**Returns:** A list of matches. Each match is a list `[target_object, verb_info]` where:

- `target_object`: The object where the matching verb was found
- `verb_info`: A list `[owner, permissions, display_names, matched_verb_name]` where:
    - `owner`: The object that owns the verb
    - `permissions`: Permission string (combination of `r`, `w`, `x`, `d`)
    - `display_names`: Full verb names concatenated with spaces (e.g., `"d*rop th*row"`)
    - `matched_verb_name`: The actual verb name that matched the command

**Notes:**

- Searches verbs in order: command_environment objects, then dobj, then iobj
- Returns first matching verb for each target; stops at first successful match overall
- Respects verb argument specifiers (dobj/prep/iobj requirements)
- Permission errors are skipped silently; continue searching other targets
- Returns empty list if no matching verbs are found

**Example:**

```moo
pc = parse_command("get lamp", {player, player.location}, true);
matches = find_command_verb(pc, {player, player.location});
foreach m in (matches) {
  {target, verbspec} = m;
  {owner, perms, names, verb} = verbspec;
  player:inform(sprintf("Found verb '%s' on %O (owner: %O)", verb, target, owner));
}
```

### `dispatch_command_verb`

```
any dispatch_command_verb(obj target, str verb_name, map parsed_command_spec)
```

Executes a command verb on a target object with full command environment (dobj, iobj, prep, etc.). This is the
lowest-level command execution function and is wizard-only.

**Arguments:**

- `target`: The object on which to execute the verb
- `verb_name`: The name of the verb to execute
- `parsed_command_spec`: A map from [`parse_command()`](#parse_command) containing command components (dobj, iobj, prep,
  argstr, etc.)

**Returns:** The return value from the executed verb

**Permissions:** Wizard-only. If the programmer is not a wizard, `E_PERM` is raised.

**Notes:**

- Bypasses the normal exec bit requirement on verbs
- Looks up the verb on the target object and validates it exists
- Uses the parsed command spec to fill all command variables (dobj, dobjstr, iobj, iobjstr, prep, prepstr, argstr, args)
- Sets caller_perms_override to `#-1` to mimic top-level command execution behavior
- Raises `E_VERBNF` if the verb is not found on the target
- Raises `E_PERM` if permission checks fail
- Raises `E_INVARG` on other lookup errors

**Example:**

```moo
// Custom command dispatcher with ambiguity resolution
parse_result = parse_command("put coin in jar", env, true);

// Get list of possible matches
matches = find_command_verb(parse_result, command_env);

if (matches) {
  // Execute the first matching verb
  {target, verb_info} = matches[1];
  {owner, perms, names, verb_name} = verb_info;

  return dispatch_command_verb(target, verb_name, parse_result);
} else {
  player:inform("I don't understand that.");
}
```

### Complete Example: Custom Command Handler

The cowbell core's `$sysobj:_command_handler()` verb demonstrates a complete implementation that handles ambiguous
object matches by trying different combinations:

```moo
verb _command_handler (this none this) owner: ARCH_WIZARD flags: "rxd"
  {command, match_env} = args;

  // Parse the command with complex matching enabled
  pc = parse_command(command, match_env, true);

  // Get verb search targets
  command_env = player:command_environment();

  // Build list of dobj/iobj candidates to try
  dobj_candidates = (pc["dobj"] == $ambiguous_match)
    ? pc["ambiguous_dobj"]
    : {pc["dobj"]};
  iobj_candidates = (pc["iobj"] == $ambiguous_match)
    ? pc["ambiguous_iobj"]
    : {pc["iobj"]};

  // Try each combination of dobj/iobj
  for dobj in (dobj_candidates) {
    for iobj in (iobj_candidates) {
      test_pc = pc;
      test_pc["dobj"] = dobj;
      test_pc["iobj"] = iobj;

      // Find verbs matching this combination
      vm_matches = find_command_verb(test_pc, command_env);

      if (vm_matches) {
        // Execute the first match
        {target, verbspec} = vm_matches[1];
        {def, flags, verbnames, v} = verbspec;
        return dispatch_command_verb(target, v, test_pc);
      }
    }
  }

  // No match found - handle "I don't understand" case
  player:inform("I don't understand that.");
  return true;
endverb
```

See [The Built-in Command Parser](../../the-built-in-command-parser.md) for an overview of how MOO command parsing
works.
