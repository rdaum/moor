# Threading

ToastStunt is single threaded, but it utilizes a threading library (extension-background) to allow certain server functions to run in a separate thread. To protect the database, these functions will implicitly suspend the MOO code (similar to how read() operates).

It is possible to disable threading of functions for a particular verb by calling `set_thread_mode(0)`.

> Note: By default, ToastStunt has threading enabled.

There are configurable options for the background subsystem which can be defined in `options.h`.

- `TOTAL_BACKGROUND_THREADS` is the total number of pthreads that will be created at runtime to process background MOO tasks.
- `DEFAULT_THREAD_MODE` dictates the default behavior of threaded MOO functions without a call to set_thread_mode. When set to true, the default behavior is to thread these functions, requiring a call to set_thread_mode(0) to disable. When false, the default behavior is unthreaded and requires a call to set_thread_mode(1) to enable threading for the functions in that verb.

When you execute a threaded built-in in your code, your code is suspended. For this reason care should be taken in how and when you use these functions with threading enabled.

Functions that support threading, and functions for utilizing threading such as `thread_pool` are discussed in the built-ins section.
