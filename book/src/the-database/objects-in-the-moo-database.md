# Objects in the MOO Database

There are anonymous objects and permanent objects in ToastStunt. Throughout this guide when we discuss `objects` we are typically referring to `permanent objects` and not `anonymous objects`. When discussing anonymous objects we will call them out specifically.

Objects encapsulate state and behavior – as they do in other object-oriented programming languages. Permanent objects are also used to represent objects in the virtual reality, like people, rooms, exits, and other concrete things. Because of this, MOO makes a bigger deal out of creating objects than it does for other kinds of values, like integers.

Numbers always exist, in a sense; you have only to write them down in order to operate on them. With permanent objects, it is different. The permanent object with number `#958` does not exist just because you write down its number. An explicit operation, the `create()` function described later, is required to bring a permanent object into existence. Once created, permanent objects continue to exist until they are explicitly destroyed by the `recycle()` function (also described later).

Anonymous objects, which are also created using `create()`, will continue to exist until the `recycle()` function is called or until there are no more references to the anonymous object.

The identifying number associated with a permanent object is unique to that object. It was assigned when the object was created and will never be reused unless `recreate()` or `reset_max_object()` are called. Thus, if we create an object and it is assigned the number `#1076`, the next object to be created (using `create()` will be assigned `#1077`, even if `#1076` is destroyed in the meantime.

> Note: The above limitation led to design of systems to manage object reuse. The `$recycler` is one example of such a system. This is **not** present in the `minimal.db` which is included in the ToastStunt source, however it is present in the latest dump of the [ToastCore DB](https://github.com/lisdude/toastcore) which is the recommended starting point for new development.

Anonymous and permanent objects are made up of three kinds of pieces that together define its behavior: _attributes_, _properties_, and _verbs_.

## Fundamental Object Attributes

There are three fundamental _attributes_ to every object:

1. A flag representing the built-in properties allotted to the object.
2. A list of object that are its parents
3. A list of the objects that are its _children_; that is, those objects for which this object is their parent.

The act of creating a character sets the player attribute of an object and only a wizard (using the function `set_player_flag()`) can change that setting. Only characters have the player bit set to 1. Only permanent objects can be players.

The parent/child hierarchy is used for classifying objects into general classes and then sharing behavior among all members of that class. For example, the ToastCore database contains an object representing a sort of "generic" room. All other rooms are _descendants_ (i.e., children or children's children, or ...) of that one. The generic room defines those pieces of behavior that are common to all rooms; other rooms specialize that behavior for their own purposes. The notion of classes and specialization is the very essence of what is meant by _object-oriented_ programming.

Only the functions `create()`, `recycle()`, `chparent()`, `chparents()`, `renumber()` and `recreate()` can change the parent and children attributes.

Below is the table representing the `flag` for the built-in properties allotted to the object. This is simply a representation of bits, and for example, the player flag is a singular bit (0x01). So the flag is actually an integer that, when in binary, represents all of the flags on the object.

```
Player:         0x01    set_player_flag()
Programmer:     0x02    .programmer
Wizard:         0x04    .wizard
Obsolete_1:     0x08    *csssssh*
Read:           0x10    .r
Write:          0x20    .w
Obsolete_2:     0x40    *csssssh*
Fertile:        0x80    .f
Anonymous:      0x100   .a
Invalid:        0x200   <destroy anonymous object>
Recycled:       0x400   <destroy anonymous object and call recycle verb>
```

## Properties on Objects

A _property_ is a named "slot" in an object that can hold an arbitrary MOO value. Every object has eleven built-in properties whose values are constrained to be of particular types. In addition, an object can have any number of other properties, none of which have type constraints. The built-in properties are as follows:

| Property   | Description                                                |
| ---------- | ---------------------------------------------------------- |
| name       | a string, the usual name for this object                   |
| owner      | an object, the player who controls access to it            |
| location   | an object, where the object is in virtual reality          |
| contents   | a list of objects, the inverse of location                 |
| last_move  | a map of an object's last location and the time() it moved |
| programmer | a bit, does the object have programmer rights?             |
| wizard     | a bit, does the object have wizard rights?                 |
| r          | a bit, is the object publicly readable?                    |
| w          | a bit, is the object publicly writable?                    |
| f          | a bit, is the object fertile?                              |
| a          | a bit, can this be a parent of anonymous objects?          |

The `name` property is used to identify the object in various printed messages. It can only be set by a wizard or by the owner of the object. For player objects, the `name` property can only be set by a wizard; this allows the wizards, for example, to check that no two players have the same name.

The `owner` identifies the object that has owner rights to this object, allowing them, for example, to change the `name` property. Only a wizard can change the value of this property.

The `location` and `contents` properties describe a hierarchy of object containment in the virtual reality. Most objects are located "inside" some other object and that other object is the value of the `location` property.

The `contents` property is a list of those objects for which this object is their location. In order to maintain the consistency of these properties, only the `move()` function is able to change them.

The `last_move` property is a map in the form `["source" -> OBJ, "time" -> TIMESTAMP]`. This is set by the server each time an object is moved.

The `wizard` and `programmer` bits are only applicable to characters, objects representing players. They control permission to use certain facilities in the server. They may only be set by a wizard.

The `r` bit controls whether or not players other than the owner of this object can obtain a list of the properties or verbs in the object.

Symmetrically, the `w` bit controls whether or not non-owners can add or delete properties and/or verbs on this object. The `r` and `w` bits can only be set by a wizard or by the owner of the object.

The `f` bit specifies whether or not this object is _fertile_, whether or not players other than the owner of this object can create new objects with this one as the parent. It also controls whether or not non-owners can use the `chparent()` or `chparents()` built-in function to make this object the parent of an existing object. The `f` bit can only be set by a wizard or by the owner of the object.

The `a` bit specifies whether or not this object can be used as a parent of an anonymous object created by a player other than the owner of this object. It works similarly to the `f` bit, but governs the creation of anonymous objects only.

All of the built-in properties on any object can, by default, be read by any player. It is possible, however, to override this behavior from within the database, making any of these properties readable only by wizards. See the chapter on server assumptions about the database for details.

As mentioned above, it is possible, and very useful, for objects to have other properties aside from the built-in ones. These can come from two sources.

First, an object has a property corresponding to every property in its parent object. To use the jargon of object-oriented programming, this is a kind of _inheritance_. If some object has a property named `foo`, then so will all of its children and thus its children's children, and so on.

Second, an object may have a new property defined only on itself and its descendants. For example, an object representing a rock might have properties indicating its weight, chemical composition, and/or pointiness, depending upon the uses to which the rock was to be put in the virtual reality.

Every defined property (as opposed to those that are built-in) has an owner and a set of permissions for non-owners. The owner of the property can get and set the property's value and can change the non-owner permissions. Only a wizard can change the owner of a property.

The initial owner of a property is the player who added it; this is usually, but not always, the player who owns the object to which the property was added. This is because properties can only be added by the object owner or a wizard, unless the object is publicly writable (i.e., its `w` property is 1), which is rare. Thus, the owner of an object may not necessarily be the owner of every (or even any) property on that object.

The permissions on properties are drawn from this set:

| Permission Bit | Description                                                   |
| -------------- | ------------------------------------------------------------- |
| `r`            | Read permission lets non-owners get the value of the property |
| `w`            | Write permission lets non-owners set the property value       |
| `c`            | Change ownership in descendants                               |

The `c` bit is a bit more complicated. Recall that every object has all of the properties that its parent does and perhaps some more. Ordinarily, when a child object inherits a property from its parent, the owner of the child becomes the owner of that property. This is because the `c` permission bit is "on" by default. If the `c` bit is not on, then the inherited property has the same owner in the child as it does in the parent.

As an example of where this can be useful, the ToastCore database ensures that every player has a `password` property containing the encrypted version of the player's connection password. For security reasons, we don't want other players to be able to see even the encrypted version of the password, so we turn off the `r` permission bit. To ensure that the password is only set in a consistent way (i.e., to the encrypted version of a player's password), we don't want to let anyone but a wizard change the property. Thus, in the parent object for all players, we made a wizard the owner of the password property and set the permissions to the empty string, `""`. That is, non-owners cannot read or write the property and, because the `c` bit is not set, the wizard who owns the property on the parent class also owns it on all of the descendants of that class.

> Warning: In classic LambdaMOO only the first 8 characters of a password were hashed. In practice this meant that the passwords `password` and `password12345` were exactly the same and either one can be used to login. This was fixed in ToastStunt. If you are upgrading from LambdaMOO, you will need to log in with only the first 8 characters of the password (and then reset your password to something more secure).

Another, perhaps more down-to-earth example arose when a character named Ford started building objects he called "radios" and another character, yduJ, wanted to own one. Ford kindly made the generic radio object fertile, allowing yduJ to create a child object of it, her own radio. Radios had a property called `channel` that identified something corresponding to the frequency to which the radio was tuned. Ford had written nice programs on radios (verbs, discussed below) for turning the channel selector on the front of the radio, which would make a corresponding change in the value of the `channel` property. However, whenever anyone tried to turn the channel selector on yduJ's radio, they got a permissions error. The problem concerned the ownership of the `channel` property.

As explained later, programs run with the permissions of their author. So, in this case, Ford's nice verb for setting the channel ran with his permissions. But, since the `channel` property in the generic radio had the `c` permission bit set, the `channel` property on yduJ's radio was owned by her. Ford didn't have permission to change it! The fix was simple. Ford changed the permissions on the `channel` property of the generic radio to be just `r`, without the `c` bit, and yduJ made a new radio. This time, when yduJ's radio inherited the `channel` property, yduJ did not inherit ownership of it; Ford remained the owner. Now the radio worked properly, because Ford's verb had permission to change the channel.

## Verbs on Objects

The final kind of piece making up an object is _verbs_. A verb is a named MOO program that is associated with a particular object. Most verbs implement commands that a player might type; for example, in the ToastCore database, there is a verb on all objects representing containers that implements commands of the form `put object in container`.

It is also possible for MOO programs to invoke the verbs defined on objects. Some verbs, in fact, are designed to be used only from inside MOO code; they do not correspond to any particular player command at all. Thus, verbs in MOO are like the _procedures_ or _methods_ found in some other programming languages.

> Note: There are even more ways to refer to _verbs_ and their counterparts in other programming language: _procedure_, _function_, _subroutine_, _subprogram_, and _method_ are the primary ones. However, in _Object Oriented Programming_ abbreviated _OOP_ you may primarily know them as methods.

As with properties, every verb has an owner and a set of permission bits. The owner of a verb can change its program, its permission bits, and its argument specifiers (discussed below). Only a wizard can change the owner of a verb.

The owner of a verb also determines the permissions with which that verb runs; that is, the program in a verb can do whatever operations the owner of that verb is allowed to do and no others. Thus, for example, a verb owned by a wizard must be written very carefully, since wizards are allowed to do just about anything.

> Warning: This is serious business. The MOO has a variety of checks in place for permissions (at the object, verb and property levels) that are all but ignored when a verb is executing with a wizard's permissions. You may want to create a non-wizard character and give them the programmer bit, and write much of your code there, leaving the wizard bit for things that actually require access to everything, despite permissions.

| Permission Bit | Description                                  |
| -------------- | -------------------------------------------- |
| r (read)       | Let non-owners see the verb code             |
| w (write)      | Let non-owners write the verb code           |
| x (execute)    | Let verb be invoked from within another verb |
| d (debug)      | Let the verb raise errors to be caught       |

The permission bits on verbs are drawn from this set: `r` (read), `w` (write), `x` (execute), and `d` (debug). Read permission lets non-owners see the program for a verb and, symmetrically, write permission lets them change that program. The other two bits are not, properly speaking, permission bits at all; they have a universal effect, covering both the owner and non-owners.

The execute bit determines whether or not the verb can be invoked from within a MOO program (as opposed to from the command line, like the `put` verb on containers). If the `x` bit is not set, the verb cannot be called from inside a program. This is most obviously useful for `this none this` verbs which are intended to be executed from within other verb programs, however, it may be useful to set the `x` bit on verbs that are intended to be executed from the command line, as then those can also
be executed from within another verb.

The setting of the debug bit determines what happens when the verb's program does something erroneous, like subtracting a number from a character string. If the `d` bit is set, then the server _raises_ an error value; such raised errors can be _caught_ by certain other pieces of MOO code. If the error is not caught, however, the server aborts execution of the command and, by default, prints an error message on the terminal of the player whose command is being executed. (See the chapter on server assumptions about the database for details on how uncaught errors are handled.) If the `d` bit is not set, then no error is raised, no message is printed, and the command is not aborted; instead the error value is returned as the result of the erroneous operation.

> Note: The `d` bit exists only for historical reasons; it used to be the only way for MOO code to catch and handle errors. With the introduction of the `try` -`except` statement and the error-catching expression, the `d` bit is no longer useful. All new verbs should have the `d` bit set, using the newer facilities for error handling if desired. Over time, old verbs written assuming the `d` bit would not be set should be changed to use the new facilities instead.

In addition to an owner and some permission bits, every verb has three _argument specifiers_, one each for the `direct object`, the `preposition`, and the `indirect object`. The direct and indirect specifiers are each drawn from this set: `this`, `any`, or `none`. The preposition specifier is `none`, `any`, or one of the items in this list:

| Preposition              |
| ------------------------ |
| with/using               |
| at/to                    |
| in front of              |
| in/inside/into           |
| on top of/on/onto/upon   |
| out of/from inside/from  |
| over                     |
| through                  |
| under/underneath/beneath |
| behind                   |
| beside                   |
| for/about                |
| is                       |
| as                       |
| off/off of               |

The argument specifiers are used in the process of parsing commands, described in the next chapter.
