# 'moor'; lambdaMOO in Rust

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

Eventual new feature goals, after full MOO backwards compatibility has been achieved:

* Embedded JavaScript engine to allow implementation of MOO verbs in a more modern standard language.
* Extended protocol support (WebSockets, HTTP, etc. inbound and outbound).
* Incremental runtime changes:
  * Remove object numbers and replace with capability references.
  * Lightweight transient object values in addition to rooted objects.
  * New primitive types in the language / properties.
   
## LambdaMOO is 30+ years old, why remain compatible?

* Because it's easy to go into the weeds creating new things, and never finishing. By having a concrete goal, and something
  to compare and test against, I may actually get somewhere.
* Because the *actual* useful and hard parts of those old MOO-type systems was the "user-space" type pieces (like
  LambdaCore/JHCore etc) and by making a new system run those old cores, there's more win.
* Because LambdaMOO itself is actually a very *complicated system with a lot of moving parts*; there's a compiler,  
  an object database, a virtual machine, a decompiler, and a network runtime all rolled into one. This, is, in some
  way... fun.

### So far ...

   * I've converted the full LambdaMOO 1.8.x grammar into an ANTLR V4 grammar. And it works to compile existing MOO
     source.
   * Compilation from ANTLR compilation tree to abstract syntax tree complete, so is theoretically ready for codgen.
   * Implementation of SQLite backed storage for the MOO object model. (The first of potentially many implementations of
     the object DB backend.) Supports verb properties, verbs, etc.
   * Capability of full import of an existing textdump into said DB.
   * About halfway through the implementation of a virtual machine 

### Next steps

   * virtual machine & bytecode generation
   * decompilation
   * network layer
#
Contributions are welcome and encouraged. 

Ryan (ryan.daum@gmail.com)
