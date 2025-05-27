# Object Properties

A _property_ is a named "slot" in an object that can hold any MOO value. Think of properties as the variables that belong to an object—they store information about what the object is and what state it's in.

**How properties work:**
- You access properties using dot notation: `object.property_name`
- You can read values: `player.score` might return `1500`
- You can set values: `sword.damage = 25`
- Properties can hold any type of data: strings, numbers, lists, other objects, etc.

**Creating and managing properties:**
- Object owners and wizards can add new properties to objects
- Properties are inherited—if a parent object has a `weight` property, all its children automatically have one too
- Children can override inherited properties with their own values
- Properties have permissions that control who can read or modify them

## Built-in properties

Every object automatically comes with several built-in properties that MOO uses for core functionality. You can't delete these, but you can read and (usually) modify them just like regular properties:

| Property   | Description                                                |
|------------|------------------------------------------------------------|
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

The `name` property is used to identify the object in various printed messages. It can only be set by a wizard or by the
owner of the object. For player objects, the `name` property can only be set by a wizard; this allows the wizards, for
example, to check that no two players have the same name.

The `owner` identifies the object that has owner rights to this object, allowing them, for example, to change the `name`
property. Only a wizard can change the value of this property.

The `location` and `contents` properties describe a hierarchy of object containment in the virtual reality. Most objects
are located "inside" some other object and that other object is the value of the `location` property.

The `contents` property is a list of those objects for which this object is their location. In order to maintain the
consistency of these properties, only the `move()` function is able to change them.

The `last_move` property is a map in the form `["source" -> OBJ, "time" -> TIMESTAMP]`. This is set by the server each
time an object is moved.

The `wizard` and `programmer` bits are only applicable to characters, objects representing players. They control
permission to use certain facilities in the server. They may only be set by a wizard.

The `r` bit controls whether or not players other than the owner of this object can obtain a list of the properties or
verbs in the object.

Symmetrically, the `w` bit controls whether or not non-owners can add or delete properties and/or verbs on this object.
The `r` and `w` bits can only be set by a wizard or by the owner of the object.

The `f` bit specifies whether or not this object is _fertile_, whether or not players other than the owner of this
object can create new objects with this one as the parent. It also controls whether or not non-owners can use the
`chparent()` or `chparents()` built-in function to make this object the parent of an existing object. The `f` bit can
only be set by a wizard or by the owner of the object.

All of the built-in properties on any object can, by default, be read by any player. It is possible, however, to
override this behavior from within the database, making any of these properties readable only by wizards. See the
chapter on server assumptions about the database for details.

## Custom properties: Building your world

The real power of MOO comes from adding your own properties to objects. This is how you create the unique characteristics that make your world interesting and interactive.

**Where custom properties come from:**

**Inheritance** - An object automatically has all the properties that its parent object has. If you create a "generic weapon" object with properties like `damage`, `weight`, and `material`, then every sword, axe, and bow that inherits from it will also have those properties.

**Direct definition** - You can add completely new properties to specific objects. For example, a magical sword might have a `magic_power` property that no other weapon has.

**Examples of custom properties:**
- A player might have: `score`, `level`, `inventory_limit`, `last_login`
- A room might have: `temperature`, `lighting`, `exits_hidden`, `background_music`
- A weapon might have: `damage`, `durability`, `enchantment`, `required_strength`

## Property ownership and permissions

Every defined property (as opposed to those that are built-in) has an owner and a set of permissions for non-owners. The
owner of the property can get and set the property's value and can change the non-owner permissions. Only a wizard can
change the owner of a property.

The initial owner of a property is the player who added it; this is usually, but not always, the player who owns the
object to which the property was added. This is because properties can only be added by the object owner or a wizard,
unless the object is publicly writable (i.e., its `w` property is 1), which is rare. Thus, the owner of an object may
not necessarily be the owner of every (or even any) property on that object.

The permissions on properties are drawn from this set:

| Permission Bit | Description                                                   |
|----------------|---------------------------------------------------------------|
| `r`            | Read permission lets non-owners get the value of the property |
| `w`            | Write permission lets non-owners set the property value       |
| `c`            | Change ownership in descendants                               |

The `c` bit is a bit more complicated. Recall that every object has all of the properties that its parent does and
perhaps some more. Ordinarily, when a child object inherits a property from its parent, the owner of the child becomes
the owner of that property. This is because the `c` permission bit is "on" by default. If the `c` bit is not on, then
the inherited property has the same owner in the child as it does in the parent.

As an example of where this can be useful, the LambdaCore database ensures that every player has a `password` property
containing the encrypted version of the player's connection password. For security reasons, we don't want other players
to be able to see even the encrypted version of the password, so we turn off the `r` permission bit. To ensure that the
password is only set in a consistent way (i.e., to the encrypted version of a player's password), we don't want to let
anyone but a wizard change the property. Thus, in the parent object for all players, we made a wizard the owner of the
password property and set the permissions to the empty string, `""`. That is, non-owners cannot read or write the
property and, because the `c` bit is not set, the wizard who owns the property on the parent class also owns it on all
of the descendants of that class.

> Warning: In classic LambdaMOO only the first 8 characters of a password were hashed. In practice this meant that the
> passwords `password` and `password12345` were exactly the same and either one can be used to login. This is not the
> case in mooR. If you are upgrading from LambdaMOO, you will need to log in with only the first 8 characters of the
> password (and then reset your password to something more secure).

Another, perhaps more down-to-earth example arose when a character named Ford started building objects he called "
radios" and another character, yduJ, wanted to own one. Ford kindly made the generic radio object fertile, allowing yduJ
to create a child object of it, her own radio. Radios had a property called `channel` that identified something
corresponding to the frequency to which the radio was tuned. Ford had written nice programs on radios (verbs, discussed
below) for turning the channel selector on the front of the radio, which would make a corresponding change in the value
of the `channel` property. However, whenever anyone tried to turn the channel selector on yduJ's radio, they got a
permissions error. The problem concerned the ownership of the `channel` property.

As explained later, programs run with the permissions of their author. So, in this case, Ford's nice verb for setting
the channel ran with his permissions. But, since the `channel` property in the generic radio had the `c` permission bit
set, the `channel` property on yduJ's radio was owned by her. Ford didn't have permission to change it! The fix was
simple. Ford changed the permissions on the `channel` property of the generic radio to be just `r`, without the `c` bit,
and yduJ made a new radio. This time, when yduJ's radio inherited the `channel` property, yduJ did not inherit ownership
of it; Ford remained the owner. Now the radio worked properly, because Ford's verb had permission to change the channel.
