# 'moor'; lambdaMOO in Rust

(note: name is provisional and awful)

## Intent
Because I don't have enough incomplete projects ...

And in the general theme that all things get to be rewritten in Rust, because that is the way of things in the 2020s.

And scratching an itch of many years to build a more modern LambdaMOO inspired thing, without actually building a more
modern LambdaMOO inspired thing and instead just building LambdaMOO itself

### Well, only partially a jest...

The intent here is to start out at least fully compatible with LambdaMOO 1.8.x series and to be able to read and
execute existing cores. 

### But then...

... to actually implement the backend portions on a more modern foundation, with a proper disk-based 
transactionally safe database and full multithreaded concurrency, and replacing the classic `telnet` 
client connectivity with websockets and such.

### So far ...

   * Successfully compiles and executes the full LambdaMOO 1.8.x language
   * Successfully imports a JaysHouseCore textdump.
   * Accepts inbound websocket connections (in lieux of telnet), attaches them to a session, and executes commands.
   * Some simple things like `say`, `emote`, `look`, `get` etc work pretty much as expected.
   * About half of builtins are supported.
   * Permissions support (though mostly untested.)
   * `fork`ed & `suspend`ed tasks

### Missing/ Next steps

   * Timeouts / tick count management in tasks
   * Auth/connect phase for the websocket server (currently just accepts any unauthed connection to any player object)
   * More builtins. See [bf_functions_status.md](bf_functions_status.md) for chart of current status.
   * Decompilation support; this is about half done.
   * Dump to textdump format. (Requires above)
   * Performance improvements. Specifically caching at the DB layer is missing and this thing will run dog slow 
     without it

## LambdaMOO is 30+ years old, why remain compatible?

* Because it's easy to go into the weeds creating new things, and never finishing. By having a concrete goal, and something
  to compare and test against, I may actually get somewhere.
* Because the *actual* useful and hard parts of those old MOO-type systems was the "user-space" type pieces (like
  LambdaCore/JHCore etc) and by making a new system run those old cores, there's more win.
* Because LambdaMOO itself is actually a very *complicated system with a lot of moving parts*; there's a compiler,  
  an object database, a virtual machine, a decompiler, and a network runtime all rolled into one. This, is, in some
  way... fun.

### Someday ...

Eventual new feature goals, after full MOO backwards compatibility has been achieved:

* Embedded JavaScript engine to allow implementation of MOO verbs in a more modern standard language.
* Extended protocol support (WebSockets, HTTP, etc. inbound and outbound).
* Distributed rather than local-only storage of objects.
* Incremental runtime / model changes:
  * Remove object numbers and replace with obj-capability references.
  * Lightweight transient object values in addition to rooted objects. (ala "WAIFs")
  * New primitive types in the language / properties.
  * all that kinda fantasy stuff

Contributions are welcome and encouraged. 

Ryan (ryan.daum@gmail.com)
