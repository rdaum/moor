# Object-Oriented Programming

One of the most important facilities in an object-oriented programming language is ability for a child object to make use of a parent's implementation of some operation, even when the child provides its own definition for that operation. The `pass()` function provides this facility in MOO.

### Function: `pass`

```
value pass(arg, ...)
```

calls the verb with the same name as the current verb but as defined on the parent of the object that defines the current verb.

Often, it is useful for a child object to define a verb that _augments_ the behavior of a verb on its parent object. For example, in the ToastCore database, the root object (which is an ancestor of every other object) defines a verb called `description` that simply returns the value of `this.description`; this verb is used by the implementation of the `look` command. In many cases, a programmer would like the
description of some object to include some non-constant part; for example, a sentence about whether or not the object was 'awake' or 'sleeping'. This sentence should be added onto the end of the normal description. The programmer would like to have a means of calling the normal `description` verb and then appending the sentence onto the end of that description. The function `pass()` is for exactly such situations.

`pass` calls the verb with the same name as the current verb but as defined on the parent of the object that defines the current verb. The arguments given to `pass` are the ones given to the called verb and the returned value of the called verb is returned from the call to `pass`. The initial value of `this` in the called verb is the same as in the calling verb.

Thus, in the example above, the child-object's `description` verb might have the following implementation:

```
return pass() + "  It is " + (this.awake ? "awake." | "sleeping.");
```

That is, it calls its parent's `description` verb and then appends to the result a sentence whose content is computed based on the value of a property on the object.

In almost all cases, you will want to call `pass()` with the same arguments as were given to the current verb. This is easy to write in MOO; just call `pass(@args)`.
