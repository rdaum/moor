# Creating and Recycling Objects

Whenever the `create()` function is used to create a new object, that object's `initialize` verb, if any, is called with no arguments. The call is simply skipped if no such verb is defined on the object.

Symmetrically, just before the `recycle()` function actually destroys an object, the object's `recycle` verb, if any, is called with no arguments. Again, the call is simply skipped if no such verb is defined on the object.

Both `create()` and `recycle()` check for the existence of an `ownership_quota` property on the owner of the newly-created or -destroyed object. If such a property exists and its value is an integer, then it is treated as a _quota_ on object ownership. Otherwise, the following two paragraphs do not apply.

The `create()` function checks whether or not the quota is positive; if so, it is reduced by one and stored back into the `ownership_quota` property on the owner. If the quota is zero or negative, the quota is considered to be exhausted and `create()` raises `E_QUOTA`.

The `recycle()` function increases the quota by one and stores it back into the `ownership_quota` property on the owner.
