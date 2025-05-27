# Creating and Recycling Objects

## Understanding object persistence

In MOO, objects are permanent and persistent—they stick around until someone explicitly destroys them. Each object gets a unique number when it's created (like #123 or #456), and this number becomes that object's permanent identity. Even if you log out, restart the server, or come back months later, object #123 will still be the same object with the same properties and verbs.

This persistence is powerful but comes with a cost: each object "takes up" a number slot. When you create a new object, the server assigns it the next available number and that number is forever associated with that object. Even if you later destroy the object with `recycle()`, that number slot remains "taken" and won't be reused (under normal circumstances). This is intentional—it prevents confusion where old references to a recycled object might accidentally point to a completely different new object.

> **Different from Python/JavaScript**
>
> If you're coming from Python, JavaScript, or similar languages, this persistence model might feel unusual. In those languages, objects automatically disappear ("get garbage collected") when nothing references them anymore. You never have to explicitly delete objects—the language handles cleanup automatically.
>
> MOO is different: objects stick around forever until you explicitly `recycle()` them, even if no variables point to them. This means you need to be more careful about cleaning up objects you no longer need.

## Creating objects

Objects are brought into existing using the `create()` function, which (usually) takes a single argument: the parent
object of the new object. The parent object is the object from which the new object will inherit properties and verbs.
(See the chapter on [Object Parents and Inheritance](object-parents-and-children.md) for more details on how inheritance
works in MOO.)

The `create()` function returns the number of the newly-created object.

Whenever the `create()` function is used to create a new object, that object's `initialize` verb, if any, is called with
no arguments. The call is simply skipped if no such verb is defined on the object.

Symmetrically, there is a `recycle()` function that destroys an object, which is usually called with a single argument:
the object
to be destroyed. Just before the `recycle()` function actually destroys an object, the object's `recycle` verb, if any,
is
called with no arguments. Again, the call is simply skipped if no such verb is defined on the object.

Permissions to create a child of an object, or to recycle an object, are controlled by the permissions and ownerships
constraints described in the [Objects in the MOO database](objects-in-the-moo-database.md) chapter. In particular,
the `create()` function will raise `E_PERM` if the caller does not have permission to create a child of the parent
object, and the `recycle()` function will raise `E_PERM` if the caller does not have permission to recycle the object
being destroyed. Documentation on `create()` and `recycle()` in
the [Manipulating Objects](../the-moo-programming-language/built-in-functions/objects.md) chapter describes
the exact permissions required for each function.
// TODO: Quota support as described below is not yet implemented in the mooR server, but may be in the future. Most
// modern cores instead implement this functionality in-core, however.

Both `create()` and `recycle()` check for the existence of an `ownership_quota` property on the owner of the
newly-created or -destroyed object. If such a property exists and its value is an integer, then it is treated as a
_quota_ on object ownership. Otherwise, the following two paragraphs do not apply.

The `create()` function checks whether or not the quota is positive; if so, it is reduced by one and stored back into
the `ownership_quota` property on the owner. If the quota is zero or negative, the quota is considered to be exhausted
and `create()` raises `E_QUOTA`.

The `recycle()` function increases the quota by one and stores it back into the `ownership_quota` property on the owner.

> **Lightweight alternatives: Flyweights**
>
> Because objects are "expensive" (they take up permanent number slots and require manual cleanup), mooR provides **flyweights** as a lightweight alternative for creating lots of small, temporary objects. Flyweights don't get object numbers, don't persist in the database, and automatically disappear when no longer needed—perfect for things like inventory items, temporary game pieces, or UI elements.
>
> For more details, see the [Flyweights section](moo-value-types.md#flyweights---lightweight-objects) in the value types documentation.
