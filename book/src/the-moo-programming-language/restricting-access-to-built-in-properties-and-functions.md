# Restricting Access to Built-in Properties and Functions

**Protected Properties**

A built-in property prop is deemed protected if $server_options.protect_prop exists and has a true value. However, no such property protections are recognized if the compilation option IGNORE_PROP_PROTECTED (see section Server Compilation Options) was set when building the server.

> Note: In previous versions of the server enabling this has significant performance costs, but that has been resolved with caching lookups, and thus this option is enabled by default in ToastStunt.

Whenever verb code attempts to read (on any object) the value of a built-in property that is protected in this way, the server raises E_PERM if the programmer is not a wizard.

**Protected Built-in Functions**

A built-in function func() is deemed protected if $server_options.protect_func exists and has a true value. If, for a given protected built-in function, a corresponding verb $bf_func() exists and its `x` bit is set, then that built-in function is also considered overridden, meaning that any call to func() from any object other than #0 will be treated as a call to $bf_func() with the same arguments, returning or raising whatever that verb returns or raises.

A call to a protected built-in function that is not overridden proceeds normally as long as either the caller is #0 or has wizard permissions; otherwise the server raises E_PERM.

Note that you must call load_server_options() in order to ensure that changes made in $server_options take effect.
