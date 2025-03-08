# ToastStunt Programmer's Manual Version 1.1.00

## Written for ToastStunt Version 2.7.0, Last Updated 03/07/23

by Pavel Curtis et al

Copyright © 1991, 1992, 1993, 1995, 1996 by Pavel Curtis.

Copyright © 1996, 1997 by Ken Fox

Copyright © 1997 by Andy Bakun

Copyright © 1997 by Erik Ostrom.

Copyright © 2004 by Roger F. Crew.

Copyright © 2011, 2012, 2013, 2014 by Todd Sundsted

Copyright © 2017-2023 by [Brendan Butts](http://github.com/sevenecks).

Copyright © 2021-2023 By [lisdude](http://github.com/lisdude)

Portions adapted from the [Stunt Programmers Manual](https://lisdude.com/moo/ProgrammersManual.html) by Todd Sundsted Copyright © 2011, 2012, 2013, 2014 by Todd Sundsted.

Portions adapted from the [WAIF documentation](http://ben.com/MOO/waif.html) and [WAIF Programmers Manual](http://ben.com/MOO/waif-progman.html) by Ben Jackson.

([CHANGE LOG](https://github.com/SevenEcks/lambda-moo-programming/blob/master/toast-stunt-programmers-guide/CHANGELOG.md)):

([CONTRIBUTORS](https://github.com/SevenEcks/lambda-moo-programming/blob/master/toast-stunt-programmers-guide/CONTRIBUTORS.md)):

Permission is granted to make and distribute verbatim copies of this manual provided the copyright notice and this permission notice are preserved on all copies.

Permission is granted to copy and distribute modified versions of this manual under the conditions for verbatim copying, provided that the entire resulting derived work is distributed under the terms of a permission notice identical to this one.

Permission is granted to copy and distribute translations of this manual into another language, under the above conditions for modified versions, except that this permission notice may be stated in a translation approved by the author.

For older versions of this document (or for pre-fork LambdaMOO version) please see the resources section.

## Table of Contents

- [Foreword](#foreword)
  * [What is ToastStunt?](#what-is-toaststunt-)
- [ToastStunt & MOO Resources](#toaststunt---moo-resources)
- [Introduction](#introduction)
- [The ToastStunt Database](#the-toaststunt-database)
  * [MOO Value Types](#moo-value-types)
    + [Integer Type](#integer-type)
    + [Real Number Type](#real-number-type)
    + [String Type](#string-type)
    + [Object Type](#object-type)
    + [Anonymous Object Type](#anonymous-object-type)
    + [Bool Type](#bool-type)
    + [WAIF Type](#waif-type)
    + [Error Type](#error-type)
    + [List Type](#list-type)
    + [Map Type](#map-type)
  * [Objects in the MOO Database](#objects-in-the-moo-database)
    + [Fundamental Object Attributes](#fundamental-object-attributes)
    + [Properties on Objects](#properties-on-objects)
    + [Verbs on Objects](#verbs-on-objects)
- [The Built-in Command Parser](#the-built-in-command-parser)
  * [Threading](#threading)
- [The MOO Programming Language](#the-moo-programming-language)
  * [MOO Language Expressions](#moo-language-expressions)
    + [Errors While Evaluating Expressions](#errors-while-evaluating-expressions)
    + [Writing Values Directly in Verbs](#writing-values-directly-in-verbs)
    + [Naming Values Within a Verb](#naming-values-within-a-verb)
    + [Arithmetic Operators](#arithmetic-operators)
    + [Bitwise Operators](#bitwise-operators)
    + [Comparing Values](#comparing-values)
    + [Values as True and False](#values-as-true-and-false)
    + [Indexing into Lists, Maps and Strings](#indexing-into-lists--maps-and-strings)
      - [Extracting an Element by Index](#extracting-an-element-by-index)
      - [Replacing an Element of a List, Map, or String](#replacing-an-element-of-a-list--map--or-string)
      - [Extracting a Subsequence of a List, Map or String](#extracting-a-subsequence-of-a-list--map-or-string)
      - [Replacing a Subsequence of a List, Map or String](#replacing-a-subsequence-of-a-list--map-or-string)
    + [Other Operations on Lists](#other-operations-on-lists)
    + [Spreading List Elements Among Variables](#spreading-list-elements-among-variables)
    + [Operations on BOOLs](#operations-on-bools)
    + [Getting and Setting the Values of Properties](#getting-and-setting-the-values-of-properties)
    + [Calling Built-in Functions and Other Verbs](#calling-built-in-functions-and-other-verbs)
    + [Verb Calls on Primitive Types](#verb-calls-on-primitive-types)
    + [Catching Errors in Expressions](#catching-errors-in-expressions)
    + [Parentheses and Operator Precedence](#parentheses-and-operator-precedence)
  * [MOO Language Statements](#moo-language-statements)
    + [Errors While Executing Statements](#errors-while-executing-statements)
    + [Simple Statements](#simple-statements)
    + [Statements for Testing Conditions](#statements-for-testing-conditions)
    + [Statements for Looping](#statements-for-looping)
    + [Terminating One or All Iterations of a Loop](#terminating-one-or-all-iterations-of-a-loop)
    + [Returning a Value from a Verb](#returning-a-value-from-a-verb)
    + [Handling Errors in Statements](#handling-errors-in-statements)
    + [Cleaning Up After Errors](#cleaning-up-after-errors)
    + [Executing Statements at a Later Time](#executing-statements-at-a-later-time)
  * [MOO Tasks](#moo-tasks)
  * [Working with Anonymous Objects](#working-with-anonymous-objects)
  * [Working with WAIFs](#working-with-waifs)
    + [The WAIF Verb and Property Namespace](#the-waif-verb-and-property-namespace)
    + [Additional Details on WAIFs](#additional-details-on-waifs)
  * [Built-in Functions](#built-in-functions)
    + [Object-Oriented Programming](#object-oriented-programming)
    + [Manipulating MOO Values](#manipulating-moo-values)
      - [General Operations Applicable to all Values](#general-operations-applicable-to-all-values)
      - [Operations on Numbers](#operations-on-numbers)
      - [Operations on Strings](#operations-on-strings)
      - [Perl Compatible Regular Expressions](#perl-compatible-regular-expressions)
      - [Legacy MOO Regular Expressions](#legacy-moo-regular-expressions)
      - [Operations on Lists](#operations-on-lists)
      - [Operations on Maps](#operations-on-maps)
    + [Manipulating Objects](#manipulating-objects)
      - [Fundamental Operations on Objects](#fundamental-operations-on-objects)
      - [Object Movement](#object-movement)
      - [Operations on Properties](#operations-on-properties)
      - [Operations on Verbs](#operations-on-verbs)
      - [Operations on WAIFs](#operations-on-waifs)
      - [Operations on Player Objects](#operations-on-player-objects)
    + [Operations on Files](#operations-on-files)
      - [Operations on SQLite](#operations-on-sqlite)
      - [Operations on The Server Environment](#operations-on-the-server-environment)
      - [Operations on Network Connections](#operations-on-network-connections)
      - [Operations Involving Times and Dates](#operations-involving-times-and-dates)
      - [MOO-Code Evaluation and Task Manipulation](#moo-code-evaluation-and-task-manipulation)
      - [Administrative Operations](#administrative-operations)
  * [Server Commands and Database Assumptions](#server-commands-and-database-assumptions)
    + [Command Lines That Receive Special Treatment](#command-lines-that-receive-special-treatment)
      - [Flushing Unprocessed Input](#flushing-unprocessed-input)
      - [Out-of-Band Processing](#out-of-band-processing)
      - [Quoted Lines](#quoted-lines)
      - [Commands](#commands)
      - [Command-Output Delimiters](#command-output-delimiters)
    + [The .program Command](#the-program-command)
    + [Initial Punctuation in Commands](#initial-punctuation-in-commands)
  * [Server Assumptions About the Database](#server-assumptions-about-the-database)
    + [Server Options Set in the Database](#server-options-set-in-the-database)
    + [Server Messages Set in the Database](#server-messages-set-in-the-database)
    + [Checkpointing the Database](#checkpointing-the-database)
  * [Networking](#networking)
    + [Accepting and Initiating Network Connections](#accepting-and-initiating-network-connections)
    + [Associating Network Connections with Players](#associating-network-connections-with-players)
    + [Player Input Handlers](#player-input-handlers)
  * [The First Tasks Run By the Server](#the-first-tasks-run-by-the-server)
  * [Controlling the Execution of Tasks](#controlling-the-execution-of-tasks)
  * [Controlling the Handling of Aborted Tasks](#controlling-the-handling-of-aborted-tasks)
  * [Matching in Command Parsing](#matching-in-command-parsing)
  * [Restricting Access to Built-in Properties and Functions](#restricting-access-to-built-in-properties-and-functions)
  * [Creating and Recycling Objects](#creating-and-recycling-objects)
  * [Object Movement](#object-movement-1)
  * [Temporarily Enabling Obsolete Server Features](#temporarily-enabling-obsolete-server-features)
  * [Signals to the Server](#signals-to-the-server)

## Foreword

01/08/2022

Hi, I'm Brendan aka Slither. I've been coding in MOO on [Sindome](https://www.sindome.org/) since 2003. I have spent many many hours over the years pouring over the original LambdaMOO programmers manual written by Pavel Curtis. In 2016 I set about updating the original manual with some of my learnings and it wasn't until 2019 that I finished work on it. Around the same time I heard about ToastStunt from [lisdude](https://github.com/lisdude). I wanted to get involved, but sadly, my C/++ coding skills are terrible. Thus, I decided my contribution would be to update the programmers guide to include all the changes that Toast has added, as well as further learnings and feedback from lisdude, DistantOrigin, and other members of the ToastStunt Discord server.

This guide is not just a technical document, it also contains opinions (mine and others) about what you should (waifs) and shouldn't (anonymous objects, multiple inheritance) consider doing and using. In the end though it's up to you. LambdaMOO and now its successor, ToastStunt, are amazing for creating games, hobby projects, and for tinkering or learning to code. I hope you have some fun!

### What is ToastStunt?

ToastStunt is a fork of [Stunt](https://github.com/toddsundsted/stunt) which is a fork of of [LambdaMOO](https://en.wikipedia.org/wiki/MOO). At the time of this writing (01/01/22) ToastStunt was under active development. Stunt and LambdaMOO were not. If you are looking for an up to date, feature rich, LambdaMOO, then ToastStunt is for you. It incorporates much of the 'patches' that were released for legacy LambdaMOO, and much more (TLS, better
FileIO, updated and expanded built-ins functions, multiple inheritance, curl support, and threading to name a few).

## ToastStunt & MOO Resources

* [LambdaMOO & ToastStunt Programming Resources GitHub](https://github.com/SevenEcks/lambda-moo-programming)
* [Newbie Guide to Compiling ToastStunt](https://lisdude.com/moo/toaststunt_newbie.txt)
* [lisdude MOO Resources](http://www.lisdude.com/moo/)
* [Unedited Original MOO Programmers Manual](http://www.hayseed.net/MOO/manuals/ProgrammersManual.html)
* [Older Unedited MOO Programmers Manual](http://www2.iath.virginia.edu/courses/moo/ProgrammersManual.texinfo_toc.html)
* [ToastStunt Source (GitHub)](https://github.com/lisdude/toaststunt)
* [ToastCore Database Repo](https://github.com/lisdude/toastcore) [ToastCore Datbase File](https://raw.githubusercontent.com/lisdude/toastcore/master/toastcore.db)
* [MOO Talk Mailing List](https://groups.google.com/forum/#!forum/MOO-talk)
* [Dome Client Web Socket MOO Client](https://github.com/JavaChilly/dome-client.js)
* [MOO FAQ](http://www.moo.mud.org/moo-faq/)
* [Arch Wizard FAQ](https://lisdude.com/moo/new-archwiz-faq.html)
* [Anatomy of a LambdaMOO DB file](https://lisdude.com/moo/lmdb.html)
* [Wizard Basics](https://lisdude.com/moo/wizbasics.html)
* [Whitepaper on Garbage Collection](https://researcher.watson.ibm.com/researcher/files/us-bacon/Bacon01Concurrent.pdf) (this was referenced when creating the garbage collector that Toast can optionally use)
* [Anatomy of ToastStunt Database](https://lisdude.com/moo/toaststunt_anatomy/)

## Introduction

ToastStunt is a network-accessible, multi-user, programmable, interactive system well-suited to the construction of text-based adventure games, conferencing systems, and other collaborative software. Its most common use, however, is as a multi-participant, low-bandwidth virtual reality, and it is with this focus in mind that I describe it here.

Participants (usually referred to as _players_) connect to ToastStunt using a telnet, SSH, or specialized [mud client](https://en.wikipedia.org/wiki/MUD_client). Upon connection, they are usually presented with a _welcome message_ explaining how to either create a new _character_ or connect to an existing one. Characters are the embodiment of players in the virtual reality that is ToastStunt.

> Note: No one really connects to a MOO using just a telnet client these days. MUD Clients are incredibly common, and can connect on the telnet (or SSH) port. See the resources section for more information on these. There are even some web based clients ([dome-client](https://github.com/JavaChilly/dome-client.js)) out there that use websockets to connect to a MOO directly from the browser. And ToastStunt can be configured to offer secure connections using TLS.

Having connected to a character, players then give one-line commands that are parsed and interpreted by ToastStunt as appropriate. Such commands may cause changes in the virtual reality, such as the location of a character, or may simply report on the current state of that reality, such as the appearance of some object.

The job of interpreting those commands is shared between the two major components in the ToastStunt system: the _server_ and the _database_.  The server is a program, written in a standard programming language, that manages the network connections, maintains queues of commands and other tasks to be executed, controls all access to the database, and executes other programs written in the MOO programming language. The database contains representations of all the objects in the virtual reality, including the MOO programs that the server executes to give those objects their specific behaviors.

Almost every command is parsed by the server into a call on a MOO procedure, or _verb_, that actually does the work. Thus, programming in the MOO language is a central part of making non-trivial extensions to the database and thus, the virtual reality.

In the next chapter, I describe the structure and contents of a ToastStunt database. The following chapter gives a complete description of how the server performs its primary duty: parsing the commands typed by players.  Next, I describe the complete syntax and semantics of the MOO programming language. Finally, I describe all of the database conventions assumed by the server.

> Note: For the most part, this manual describes only those aspects of ToastStunt that are entirely independent of the contents of the database. It does not describe, for example, the commands or programming interfaces present in the ToastCore database. There are exceptions to this, for situations where it seems prudent to delve deeper into these areas.

## The ToastStunt Database

In this chapter, we describe in detail the various kinds of data that can appear in a ToastStunt database and that MOO programs can manipulate. In a few places, we refer to the [ToastCore](https://github.com/lisdude/toastcore) database. This is one particular ToastStunt database which is under active development by the ToastStunt community.

### MOO Value Types

There are only a few kinds of values that MOO programs can manipulate:

* integers (in a specific, large range)
* real numbers (represented with floating-point numbers)
* strings (of characters)
* object numbers (of the permanent objects in the database) 
* object references (to the anonymous objects in the database)
* bools
* WAIFs
* errors (arising during program execution)
* lists (of all of the above, including lists)
* maps (of all of the above, including lists and maps)

#### Integer Type

ToastStunt supports 64 bit integers, but it can also be configured to support 32 bit. In MOO programs, integers are written just as you see them here, an optional minus sign followed by a non-empty sequence of decimal digits. In particular, you may not put commas, periods, or spaces in the middle of large integers, as we sometimes do in English and other natural languages (e.g. 2,147,483,647).

> Note: The values $maxint and $minint define in the database the maximum integers supported. These are set automatically with ToastCore. If you are migrating from LambdaMOO it is still a good idea to check that these numbers are being set properly.

#### Real Number Type

Real numbers in MOO are represented as they are in almost all other programming languages, using so-called _floating-point_ numbers. These have certain (large) limits on size and precision that make them useful for a wide range of applications. Floating-point numbers are written with an optional minus sign followed by a non-empty sequence of digits punctuated at some point with a decimal point '.' and/or followed by a scientific-notation marker (the letter 'E' or 'e' followed by an optional sign and one or more digits). Here are some examples of floating-point numbers:

```
325.0   325.  3.25e2   0.325E3   325.E1   .0325e+4   32500e-2
```

All of these examples mean the same number. The third of these, as an example of scientific notation, should be read "3.25 times 10 to the power of 2".

Fine point: The MOO represents floating-point numbers using the local meaning of the C-language `double` type, which is almost always equivalent to IEEE 754 double precision floating point. If so, then the smallest positive floating-point number is no larger than `2.2250738585072014e-308` and the largest floating-point number is `1.7976931348623157e+308`.

* IEEE infinities and NaN values are not allowed in MOO.
* The error `E_FLOAT` is raised whenever an infinity would otherwise be computed.
* The error `E_INVARG` is raised whenever a NaN would otherwise arise.
* The value `0.0` is always returned on underflow.

#### String Type

Character _strings_ are arbitrarily-long sequences of normal, ASCII printing characters. When written as values in a program, strings are enclosed in double-quotes, like this:

```
"This is a character string."
```

To include a double-quote in the string, precede it with a backslash (`\`), like this:

```
"His name was \"Leroy\", but nobody ever called him that."
```

Finally, to include a backslash in a string, double it:

```
"Some people use backslash ('\\') to mean set difference."
```

MOO strings may not include special ASCII characters like carriage-return, line-feed, bell, etc. The only non-printing characters allowed are spaces and tabs.

Fine point: There is a special kind of string used for representing the arbitrary bytes used in general, binary input and output. In a _binary string_, any byte that isn't an ASCII printing character or the space character is represented as the three-character substring "\~XX", where XX is the hexadecimal representation of the byte; the input character '~' is represented by the three-character substring "~7E". This special representation is used by the functions `encode_binary()` and `decode_binary()` and by the functions `notify()` and `read()` with network connections that are in binary mode. See the descriptions of the `set_connection_option()`, `encode_binary()`, and `decode_binary()` functions for more details.

MOO strings can be 'indexed into' using square braces and an integer index (much the same way you can with lists):

```
"this is a string"[4] -> "s"
```

There is syntactic sugar that allows you to do:
```
"Sli" in "Slither"
```
as a shortcut for the index() built-in function.

#### Object Type

_Objects_ are the backbone of the MOO database and, as such, deserve a great deal of discussion; the entire next section is devoted to them. For now, let it suffice to say that every object has a number, unique to that object.

In programs, we write a reference to a particular object by putting a hash mark (`#`) followed by the number, like this:

```
#495
```

> Note: Referencing object numbers in your code should be discouraged. An object only exists until it is recycled. It is technically possible for an object number to change under some circumstances. Thus, you should use a corified reference to an object ($my_special_object) instead. More on corified references later.

Object numbers are always integers.

There are three special object numbers used for a variety of purposes: `#-1`, `#-2`, and `#-3`, usually referred to in the ToastCore database as `$nothing`, `$ambiguous_match`, and `$failed_match`, respectively.

#### Anonymous Object Type

Anonymous Objects are references and do not have an object number. They are created by passing the optional third argument to `create()`. Anonymous objects are automatically garbage collected when there is no longer any references to them (in your code or in properties).

We will go into more detail on Anonymous Objects in the [Working with Anonymous Objects](#working-with-anonymous-objects) section.

#### Bool Type

_bools_ are either true or false. Eg: `my_bool = true; my_second_bool = false;`. In MOO `true` evaluates to `1` and `false` evaluates to `0`. For example:

```
false == 0 evaluates to true
true  == 1 evaluates to true
false == 1 evaluates to false
true  == 0 evaluates to false
true  == 5 evaluates to false
false == -43 evaluates to false
```

#### WAIF Type
_WAIFs_ are lightweight objects. A WAIF is a value which you can store in a property or a variable or inside a LIST or another WAIF. A WAIF is smaller in size (measured in bytes) than a regular object, and it is faster to create and destroy. It is also reference counted, which means it is destroyed automatically when it is no longer in use. An empty WAIF is 72 bytes, empty list is 64 bytes. A WAIF will always be 8 bytes larger than a LIST (on 64bit, 4 bytes on 32bit) with the same values in it. 

> Note: WAIFs are not truly objects and don't really function like one. You can't manipulate a WAIF without basically recreating a normal object (and then what's the point?). It may be better to think of a WAIF as another data type. It's closer to being a list than it is to being an object. But that's semantics, really.

WAIFs are smaller than typical objects, and faster to create. A WAIF has two builtin OBJ properties, .class and .owner. A WAIF is only ever going to be 4 bytes larger than a LIST with the same values.

OBJs grow by value_bytes(value) - value_bytes(0) for every property you set (that is, every property which becomes non-clear and takes on its own value distinct from the parent). LISTs and WAIFs both grow by value_bytes(value) for each new list element (in a LIST) or each property you set (in a WAIF). So a WAIF is never more than 4 bytes larger than a LIST which holds the same values, except WAIFs give each value a name (property name) but LISTs only give them numbers.

Essentially you should consider a WAIF as something you can make thousands of in a verb without a second thought. You might make a mailing list with 1000 messages, each a WAIF (instead of a LIST) but you most likely wouldn't use 1000 objects.

You create and destroy OBJs explicitly with the builtins create() and recycle() (or allocate them from a pool using verbs in the core). They stay around no matter what you do until you destroy them.

All of the other types you use in MOO (that require allocated memory) are reference counted. However you create them, they stay around as long as you keep them in a property or a variable somewhere, and when they are no longer used, they silently disappear, and you can't get them back. 

We will go into more detail on WAIFs in the [Working with WAIFs](#working-with-waifs) section.

#### Error Type

_Errors_ are, by far, the least frequently used values in MOO. In the normal case, when a program attempts an operation that is erroneous for some reason (for example, trying to add a number to a character string), the server stops running the program and prints out an error message. However, it is possible for a program to stipulate that such errors should not stop execution; instead, the server should just let the value of the operation be an error value. The program can then test for such a result and take some appropriate kind of recovery action. In programs, error values are written as words beginning with `E_`. The complete list of error values, along with their associated messages, is as follows:

| Error     | Description                     |
| --------- | ------------------------------- |
| E_NONE    | No error                        |
| E_TYPE    | Type mismatch                   |
| E_DIV     | Division by zero                |
| E_PERM    | Permission denied               |
| E_PROPNF  | Property not found              |
| E_VERBNF  | Verb not found                  |
| E_VARNF   | Variable not found              |
| E_INVIND  | Invalid indirection             |
| E_RECMOVE | Recursive move                  |
| E_MAXREC  | Too many verb calls             |
| E_RANGE   | Range error                     |
| E_ARGS    | Incorrect number of arguments   |
| E_NACC    | Move refused by destination     |
| E_INVARG  | Invalid argument                |
| E_QUOTA   | Resource limit exceeded         |
| E_FLOAT   | Floating-point arithmetic error |
| E_FILE    | File system error               |
| E_EXEC    | Exec error                      |
| E_INTRPT  | Interrupted                     |

#### List Type

Another important value in MOO programs is _lists_. A list is a sequence of arbitrary MOO values, possibly including other lists. In programs, lists are written in mathematical set notation with each of the elements written out in order, separated by commas, the whole enclosed in curly braces (`{` and `}`). For example, a list of the names of the days of the week is written like this:

```
{"Sunday", "Monday", "Tuesday", "Wednesday",
 "Thursday", "Friday", "Saturday"}
```

> Note: It doesn't matter that we put a line-break in the middle of the list. This is true in general in MOO: anywhere that a space can go, a line-break can go, with the same meaning. The only exception is inside character strings, where line-breaks are not allowed.

#### Map Type

The final type in MOO is a _map_. It is sometimes called a hashmap, associative array, or dictionary in other programming languages. A map is written as a set of key -> value pairs, for example: `["key" -> "value", 0 -> {}, #15840 -> []]`. Keys must be unique.

The key of a map can be:

* string
* integer
* object
* error
* float
* anonymous object (not recommended)
* waif
* bool

The value of a map can be any valid MOO type including another map.

> Note: Finding a value in a list is BigO(n) as a it uses a linear search. Maps are much more effective and are BigO(1) for retrieving a specific value by key.

### Objects in the MOO Database

There are anonymous objects and permanent objects in ToastStunt. Throughout this guide when we discuss `objects` we are typically referring to `permanent objects` and not `anonymous objects`. When discussing anonymous objects we will call them out specifically.

Objects encapsulate state and behavior – as they do in other object-oriented programming languages. Permanent objects are also used to represent objects in the virtual reality, like people, rooms, exits, and other concrete things. Because of this, MOO makes a bigger deal out of creating objects than it does for other kinds of values, like integers. 

Numbers always exist, in a sense; you have only to write them down in order to operate on them. With permanent objects, it is different. The permanent object with number `#958` does not exist just because you write down its number. An explicit operation, the `create()` function described later, is required to bring a permanent object into existence. Once created, permanent objects continue to exist until they are explicitly destroyed by the `recycle()` function (also described later). 

Anonymous objects, which are also created using `create()`, will continue to exist until the `recycle()` function is called or until there are no more references to the anonymous object.

The identifying number associated with a permanent object is unique to that object. It was assigned when the object was created and will never be reused unless `recreate()` or `reset_max_object()` are called. Thus, if we create an object and it is assigned the number `#1076`, the next object to be created (using `create()` will be assigned `#1077`, even if `#1076` is destroyed in the meantime.

> Note: The above limitation led to design of systems to manage object reuse. The `$recycler` is one example of such a system. This is **not** present in the `minimal.db` which is included in the ToastStunt source, however it is present in the latest dump of the [ToastCore DB](https://github.com/lisdude/toastcore) which is the recommended starting point for new development.

Anonymous and permanent objects are made up of three kinds of pieces that together define its behavior: _attributes_, _properties_, and _verbs_.

#### Fundamental Object Attributes

There are three fundamental _attributes_ to every object:

1. A flag representing the built-in properties allotted to the object. 
2. A list of object that are its parents
3. A list of the objects that are its _children_; that is, those objects for which this object is their parent.

The act of creating a character sets the player attribute of an object and only a wizard (using the function `set_player_flag()`) can change that setting. Only characters have the player bit set to 1. Only permanent objects can be players.

The parent/child hierarchy is used for classifying objects into general classes and then sharing behavior among all members of that class. For example, the ToastCore database contains an object representing a sort of "generic" room.  All other rooms are _descendants_ (i.e., children or children's children, or ...) of that one. The generic room defines those pieces of behavior that are common to all rooms; other rooms specialize that behavior for their own purposes. The notion of classes and specialization is the very essence of what is meant by _object-oriented_ programming. 

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

#### Properties on Objects

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

> Warning:  In classic LambdaMOO only the first 8 characters of a password were hashed. In practice this meant that the passwords `password` and `password12345` were exactly the same and either one can be used to login. This was fixed in ToastStunt. If you are upgrading from LambdaMOO, you will need to log in with only the first 8 characters of the password (and then reset your password to something more secure).

Another, perhaps more down-to-earth example arose when a character named Ford started building objects he called "radios" and another character, yduJ, wanted to own one. Ford kindly made the generic radio object fertile, allowing yduJ to create a child object of it, her own radio. Radios had a property called `channel` that identified something corresponding to the frequency to which the radio was tuned. Ford had written nice programs on radios (verbs, discussed below) for turning the channel selector on the front of the radio, which would make a corresponding change in the value of the `channel` property. However, whenever anyone tried to turn the channel selector on yduJ's radio, they got a permissions error. The problem concerned the ownership of the `channel` property.

As explained later, programs run with the permissions of their author. So, in this case, Ford's nice verb for setting the channel ran with his permissions.  But, since the `channel` property in the generic radio had the `c` permission bit set, the `channel` property on yduJ's radio was owned by her. Ford didn't have permission to change it!  The fix was simple. Ford changed the permissions on the `channel` property of the generic radio to be just `r`, without the `c` bit, and yduJ made a new radio. This time, when yduJ's radio inherited the `channel` property, yduJ did not inherit ownership of it; Ford remained the owner. Now the radio worked properly, because Ford's verb had permission to change the channel.

#### Verbs on Objects

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

The setting of the debug bit determines what happens when the verb's program does something erroneous, like subtracting a number from a character string.  If the `d` bit is set, then the server _raises_ an error value; such raised errors can be _caught_ by certain other pieces of MOO code. If the error is not caught, however, the server aborts execution of the command and, by default, prints an error message on the terminal of the player whose command is being executed. (See the chapter on server assumptions about the database for details on how uncaught errors are handled.)  If the `d` bit is not set, then no error is raised, no message is printed, and the command is not aborted; instead the error value is returned as the result of the erroneous operation.

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

## The Built-in Command Parser

The MOO server is able to do a small amount of parsing on the commands that a player enters. In particular, it can break apart commands that follow one of the following forms:

* verb
* verb direct-object
* verb direct-object preposition indirect-object

Real examples of these forms, meaningful in the ToastCore database, are as follows:

```
look
take yellow bird
put yellow bird in cuckoo clock
```

Note that English articles (i.e., `the`, `a`, and `an`) are not generally used in MOO commands; the parser does not know that they are not important parts of objects' names.

To have any of this make real sense, it is important to understand precisely how the server decides what to do when a player types a command.

But first, we mention the three situations in which a line typed by a player is not treated as an ordinary command:

1. The line may exactly match the connection's defined flush command, if any (`.flush` by default), in which case all pending lines of input are cleared and nothing further is done with the flush command itself. Likewise, any line may be flushed by a subsequent flush command before the server otherwise gets a chance to process it. For more on this, see Flushing Unprocessed Input.
2. The line may begin with a prefix that qualifies it for out-of-band processing and thence, perhaps, as an out-of-band command. For more on this, see Out-of-band Processing.
3. The connection may be subject to a read() call (see section Operations on Network Connections) or there may be a .program command in progress (see section The .program Command), either of which will consume the line accordingly. Also note that if connection option "hold-input" has been set, all in-band lines typed by the player are held at this point for future reading, even if no reading task is currently active. 

Otherwise, we (finally) have an actual command line that can undergo normal command parsing as follows:

The server checks whether or not the first non-blank character in the command is one of the following: 

* `"`
* `:`
* `;`

If so, that character is replaced by the corresponding command below, followed by a space:

* `say`
* `emote`
* `eval`

For example this command:

```
"Hi, there.
```

will be treated exactly as if it were as follows:

```
say Hi, there.
```

The server next breaks up the command into words. In the simplest case, the command is broken into words at every run of space characters; for example, the command `foo bar baz` would be broken into the words `foo`, `bar`, and `baz`. To force the server to include spaces in a "word", all or part of a word can be enclosed in double-quotes. For example, the command:

```
foo "bar mumble" baz" "fr"otz" bl"o"rt
```

is broken into the words `foo`, `bar mumble`, `baz frotz`, and `blort`.

Finally, to include a double-quote or a backslash in a word, they can be preceded by a backslash, just like in MOO strings.

Having thus broken the string into words, the server next checks to see if the first word names any of the six "built-in" commands:

* `.program`
* `PREFIX`
* `OUTPUTPREFIX`
* `SUFFIX`
* `OUTPUTSUFFIX`
* or the connection's defined _flush_ command, if any (`.flush` by default).

The first one of these is only available to programmers, the next four are intended for use by client programs, and the last can vary from database to database or even connection to connection; all six are described in the final chapter of this document, "Server Commands and Database Assumptions". If the first word isn't one of the above, then we get to the usual case: a normal MOO command.

The server next gives code in the database a chance to handle the command. If the verb `$do_command()` exists, it is called with the words of the command passed as its arguments and `argstr` set to the raw command typed by the user. If `$do_command()` does not exist, or if that verb-call completes normally (i.e., without suspending or aborting) and returns a false value, then the built-in command parser is invoked to handle the command as described below. Otherwise, it is assumed that the database code handled the command completely and no further action is taken by the server for that command.

> Note: `$do_command` is a corified reference. It refers to the verb `do_command` on #0. More details on corifying properties and verbs are presented later. 

If the built-in command parser is invoked, the server tries to parse the command into a verb, direct object, preposition and indirect object. The first word is taken to be the verb. The server then tries to find one of the prepositional phrases listed at the end of the previous section, using the match that occurs earliest in the command. For example, in the very odd command `foo as bar to baz`, the server would take `as` as the preposition, not `to`.

If the server succeeds in finding a preposition, it considers the words between the verb and the preposition to be the direct object and those after the preposition to be the indirect object. In both cases, the sequence of words is turned into a string by putting one space between each pair of words. Thus, in the odd command from the previous paragraph, there are no words in the direct object (i.e., it is considered to be the empty string, `""`) and the indirect object is `"bar to baz"`.

If there was no preposition, then the direct object is taken to be all of the words after the verb and the indirect object is the empty string.

The next step is to try to find MOO objects that are named by the direct and indirect object strings.

First, if an object string is empty, then the corresponding object is the special object `#-1` (aka `$nothing` in ToastCore). If an object string has the form of an object number (i.e., a hash mark (`#`) followed by digits), and the object with that number exists, then that is the named object. If the object string is either `"me"` or `"here"`, then the player object itself or its location is used, respectively.

> Note: $nothing is considered a `corified` object.  This means that a _property_ has been created on `#0` named `nothing` with the value of `#-1`. For example (after creating the property): `;#0.nothing = #-1` This allows you to reference the `#-1` object via it's corified reference of `$nothing`. In practice this can be very useful as you can use corified references in your code (and should!) instead of object numbers. Among other benefits this allows you to write your code (which references other objects) once and then swap out the corified reference, pointing to a different object. For instance if you have a new error logging system and you want to replace the old $error_logger reference with your new one, you wont have to find all the references to the old error logger object number in your code. You can just change the property on `#0` to reference the new object.  

Otherwise, the server considers all of the objects whose location is either the player (i.e., the objects the player is "holding", so to speak) or the room the player is in (i.e., the objects in the same room as the player); it will try to match the object string against the various names for these objects.

The matching done by the server uses the `aliases` property of each of the objects it considers. The value of this property should be a list of strings, the various alternatives for naming the object. If it is not a list, or the object does not have an `aliases` property, then the empty list is used.  In any case, the value of the `name` property is added to the list for the purposes of matching.

The server checks to see if the object string in the command is either exactly equal to or a prefix of any alias; if there are any exact matches, the prefix matches are ignored. If exactly one of the objects being considered has a matching alias, that object is used. If more than one has a match, then the special object `#-2` (aka `$ambiguous_match` in ToastCore) is used.  If there are no matches, then the special object `#-3` (aka `$failed_match` in ToastCore) is used.

So, now the server has identified a verb string, a preposition string, and direct- and indirect-object strings and objects. It then looks at each of the verbs defined on each of the following four objects, in order:

1. the player who typed the command
2. the room the player is in
3. the direct object, if any
4. the indirect object, if any.

For each of these verbs in turn, it tests if all of the the following are true:

* the verb string in the command matches one of the names for the verb
* the direct- and indirect-object values found by matching are allowed by the corresponding _argument specifiers_    for the verb
* the preposition string in the command is matched by the _preposition specifier_ for the verb.

I'll explain each of these criteria in turn.

Every verb has one or more names; all of the names are kept in a single string, separated by spaces. In the simplest case, a verb-name is just a word made up of any characters other than spaces and stars (i.e., ' ' and `*`). In this case, the verb-name matches only itself; that is, the name must be matched exactly.

If the name contains a single star, however, then the name matches any prefix of itself that is at least as long as the part before the star. For example, the verb-name `foo*bar` matches any of the strings `foo`, `foob`, `fooba`, or `foobar`; note that the star itself is not considered part of the name.

If the verb name _ends_ in a star, then it matches any string that begins with the part before the star. For example, the verb-name `foo*` matches any of the strings `foo`, `foobar`, `food`, or `foogleman`, among many others. As a special case, if the verb-name is `*` (i.e., a single star all by itself), then it matches anything at all.

Recall that the argument specifiers for the direct and indirect objects are drawn from the set `none`, `any`, and `this`. If the specifier is `none`, then the corresponding object value must be `#-1` (aka `$nothing` in ToastCore); that is, it must not have been specified. If the specifier is `any`, then the corresponding object value may be anything at all. Finally, if the specifier is `this`, then the corresponding object value must be the same as the object on which we found this verb; for example, if we are considering verbs on the player, then the object value must be the player object.

Finally, recall that the argument specifier for the preposition is either `none`, `any`, or one of several sets of prepositional phrases, given above. A specifier of `none` matches only if there was no preposition found in the command. A specifier of `any` always matches, regardless of what preposition was found, if any. If the specifier is a set of prepositional phrases, then the one found must be in that set for the specifier to match.

So, the server considers several objects in turn, checking each of their verbs in turn, looking for the first one that meets all of the criteria just explained. If it finds one, then that is the verb whose program will be executed for this command. If not, then it looks for a verb named `huh` on the room that the player is in; if one is found, then that verb will be called. This feature is useful for implementing room-specific command parsing or error recovery. If the server can't even find a `huh` verb to run, it prints an error message like `I couldn't understand that.` and the command is considered complete.

At long last, we have a program to run in response to the command typed by the player. When the code for the program begins execution, the following built-in variables will have the indicated values:

| Variable | Value                                                    |
| -------- | -------------------------------------------------------- |
| player   | an object, the player who typed the command              |
| this     | an object, the object on which this verb was found       |
| caller   | an object, the same as <code>player</code>               |
| verb     | a string, the first word of the command                  |
| argstr   | a string, everything after the first word of the command |
| args     | a list of strings, the words in <code>argstr</code>      |
| dobjstr  | a string, the direct object string found during parsing  |
| dobj     | an object, the direct object value found during matching |
| prepstr  | a string, the prepositional phrase found during parsing  |
| iobjstr  | a string, the indirect object string                     |
| iobj     | an object, the indirect object value                     |


The value returned by the program, if any, is ignored by the server.

### Threading

ToastStunt is single threaded, but it utilizes a threading library (extension-background) to allow certain server functions to run in a separate thread. To protect the database, these functions will implicitly suspend the MOO code (similar to how read() operates).

It is possible to disable threading of functions for a particular verb by calling `set_thread_mode(0)`.

> Note: By default, ToastStunt has threading enabled.

There are configurable options for the background subsystem which can be defined in `options.h`. 

* `TOTAL_BACKGROUND_THREADS` is the total number of pthreads that will be created at runtime to process background MOO tasks.  
* `DEFAULT_THREAD_MODE` dictates the default behavior of threaded MOO functions without a call to set_thread_mode. When set to true, the default behavior is to thread these functions, requiring a call to set_thread_mode(0) to disable.  When false, the default behavior is unthreaded and requires a call to set_thread_mode(1) to enable threading for the functions in that verb.

When you execute a threaded built-in in your code, your code is suspended. For this reason care should be taken in how and when you use these functions with threading enabled.

Functions that support threading, and functions for utilizing threading such as `thread_pool` are discussed in the built-ins section.

## The MOO Programming Language

MOO stands for "MUD, Object Oriented."  MUD, in turn, has been said to stand for many different things, but I tend to think of it as "Multi-User Dungeon" in the spirit of those ancient precursors to MUDs, Adventure and Zork.

MOO, the programming language, is a relatively small and simple object-oriented language designed to be easy to learn for most non-programmers; most complex systems still require some significant programming ability to accomplish, however.

Having given you enough context to allow you to understand exactly what MOO code is doing, I now explain what MOO code looks like and what it means. I begin with the syntax and semantics of expressions, those pieces of code that have values. After that, I cover statements, the next level of structure up from expressions. Next, I discuss the concept of a task, the kind of running process initiated by players entering commands, among other causes. Finally, I list all of the built-in functions available to MOO code and describe what they do.

First, though, let me mention comments. You can include bits of text in your MOO program that are ignored by the server. The idea is to allow you to put in notes to yourself and others about what the code is doing. To do this, begin the text of the comment with the two characters `/*` and end it with the two characters `*/`; this is just like comments in the C programming language. Note that the server will completely ignore that text; it will _not_ be saved in the database. Thus, such comments are only useful in files of code that you maintain outside the database.

To include a more persistent comment in your code, try using a character string literal as a statement. For example, the sentence about peanut butter in the following code is essentially ignored during execution but will be maintained in the database:

```
for x in (players())
  "Grendel eats peanut butter!";
  player:tell(x.name, " (", x, ")");
endfor
```

> Note: In practice, the only style of comments you will use is quoted strings of text. Get used to it. Another thing of note is that these strings ARE evaluated. Nothing is done with the results of the evaluation, because the value is not stored anywhere-- however, it may be prudent to keep string comments out of nested loops to make your code a bit faster.

### MOO Language Expressions

Expressions are those pieces of MOO code that generate values; for example, the MOO code

```
3 + 4
```

is an expression that generates (or "has" or "returns") the value 7.  There are many kinds of expressions in MOO, all of them discussed below.

#### Errors While Evaluating Expressions

Most kinds of expressions can, under some circumstances, cause an error to be generated. For example, the expression `x / y` will generate the error `E_DIV` if `y` is equal to zero. When an expression generates an error, the behavior of the server is controlled by setting of the `d` (debug) bit on the verb containing that expression. If the `d` bit is not set, then the error is effectively squelched immediately upon generation; the error value is simply returned as the value of the expression that generated it.

> Note: This error-squelching behavior is very error prone, since it affects _all_ errors, including ones the programmer may not have anticipated. The `d` bit exists only for historical reasons; it was once the only way for MOO programmers to catch and handle errors. The error-catching expression and the `try` -`except` statement, both described below, are far better ways of accomplishing the same thing.

If the `d` bit is set, as it usually is, then the error is _raised_ and can be caught and handled either by code surrounding the expression in question or by verbs higher up on the chain of calls leading to the current verb. If the error is not caught, then the server aborts the entire task and, by default, prints a message to the current player. See the descriptions of the error-catching expression and the `try`-`except` statement for the details of how errors can be caught, and the chapter on server assumptions about the database for details on the handling of uncaught errors.

#### Writing Values Directly in Verbs

The simplest kind of expression is a literal MOO value, just as described in the section on values at the beginning of this document.  For example, the following are all expressions:

* 17
* #893
* "This is a character string."
* E_TYPE
* ["key" -> "value"]
* {"This", "is", "a", "list", "of", "words"}

In the case of lists, like the last example above, note that the list expression contains other expressions, several character strings in this case. In general, those expressions can be of any kind at all, not necessarily literal values. For example,

```
{3 + 4, 3 - 4, 3 * 4}
```

is an expression whose value is the list `{7, -1, 12}`.

#### Naming Values Within a Verb

As discussed earlier, it is possible to store values in properties on objects; the properties will keep those values forever, or until another value is explicitly put there. Quite often, though, it is useful to have a place to put a value for just a little while. MOO provides local variables for this purpose.

Variables are named places to hold values; you can get and set the value in a given variable as many times as you like. Variables are temporary, though; they only last while a particular verb is running; after it finishes, all of the variables given values there cease to exist and the values are forgotten.

Variables are also "local" to a particular verb; every verb has its own set of them. Thus, the variables set in one verb are not visible to the code of other verbs.

The name for a variable is made up entirely of letters, digits, and the underscore character (`_`) and does not begin with a digit. The following are all valid variable names:

* foo
* _foo
* this2that
* M68000
* two_words
* This_is_a_very_long_multiword_variable_name

Note that, along with almost everything else in MOO, the case of the letters in variable names is insignificant. For example, these are all names for the same variable:

* fubar
* Fubar
* FUBAR
* fUbAr

A variable name is itself an expression; its value is the value of the named variable. When a verb begins, almost no variables have values yet; if you try to use the value of a variable that doesn't have one, the error value `E_VARNF` is raised. (MOO is unlike many other programming languages in which one must _declare_ each variable before using it; MOO has no such declarations.)  The following variables always have values:

| Variable |
| -------- |
| INT      |
| NUM      |
| FLOAT    |
| OBJ      |
| STR      |
| LIST     |
| ERR      |
| BOOL     |
| MAP      |
| WAIF     |
| ANON     |
| true     |
| false    |
| player   |
| this     |
| caller   |
| verb     |
| args     |
| argstr   |
| dobj     |
| dobjstr  |
| prepstr  |
| iobj     |
| iobjstr  |

> Note: `num` is a deprecated reference to `int` and has been presented only for completeness.

The values of some of these variables always start out the same:

| Variable           | Value | Description                                           |
| ------------------ | ----- | ----------------------------------------------------- |
| <code>INT</code>   | 0     | an integer, the type code for integers                |
| <code>NUM</code>   | 0     | (deprecated) an integer, the type code for integers   |
| <code>OBJ</code>   | 1     | an integer, the type code for objects                 |
| <code>STR</code>   | 2     | an integer, the type code for strings                 |
| <code>ERR</code>   | 3     | an integer, the type code for error values            |
| <code>LIST</code>  | 4     | an integer, the type code for lists                   |
| <code>FLOAT</code> | 9     | an integer, the type code for floating-point numbers  |
| <code>MAP</code>   | 10    | an integer, the type code for map values              |
| <code>ANON</code>  | 12    | an integer, the type code for anonymous object values |
| <code>WAIF</code>  | 13    | an integer, the type code for WAIF values             |
| <code>BOOL</code>  | 14    | an integer, the type code for bool values             |
| <code>true</code>  | true  | the boolean true                                      |
| <code>false</code> | false | the boolean false                                     |

> Note: The `typeof` function can is of note here and is described in the built-ins section.

For others, the general meaning of the value is consistent, though the value itself is different for different situations:

| Variable            | Value                                                                                                                                                                                                   |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <code>player</code> | an object, the player who typed the command that started the task that involved running this piece of code.                                                                                             |
| <code>this</code>   | an object, the object on which the currently-running verb was found.                                                                                                                                    |
| <code>caller</code> | an object, the object on which the verb that called the currently-running verb was found. For the first verb called for a given command, <code>caller</code> has the same value as <code>player</code>. |
| <code>verb</code>   | a string, the name by which the currently-running verb was identified.                                                                                                                                  |
| <code>args</code>   | a list, the arguments given to this verb. For the first verb called for a given command, this is a list of strings, the words on the command line.                                                      |

The rest of the so-called "built-in" variables are only really meaningful for the first verb called for a given command. Their semantics is given in the discussion of command parsing, above.

To change what value is stored in a variable, use an _assignment_ expression:

```
variable = expression
```

For example, to change the variable named `x` to have the value 17, you would write `x = 17` as an expression. An assignment expression does two things:

* it changes the value of of the named variable
* it returns the new value of that variable

Thus, the expression

```
13 + (x = 17)
```

changes the value of `x` to be 17 and returns 30.

#### Arithmetic Operators

All of the usual simple operations on numbers are available to MOO programs:

```
+
-
*
/
%
```

These are, in order, addition, subtraction, multiplication, division, and remainder. In the following table, the expressions on the left have the corresponding values on the right:

```
5 + 2       =>   7
5 - 2       =>   3
5 * 2       =>   10
5 / 2       =>   2
5.0 / 2.0   =>   2.5
5 % 2       =>   1
5.0 % 2.0   =>   1.0
5 % -2      =>   1
-5 % 2      =>   -1
-5 % -2     =>   -1
-(5 + 2)    =>   -7
```

Note that integer division in MOO throws away the remainder and that the result of the remainder operator (`%`) has the same sign as the left-hand operand. Also, note that `-` can be used without a left-hand operand to negate a numeric expression.

Fine point: Integers and floating-point numbers cannot be mixed in any particular use of these arithmetic operators; unlike some other programming languages, MOO does not automatically coerce integers into floating-point numbers. You can use the `tofloat()` function to perform an explicit conversion.

The `+` operator can also be used to append two strings. The expression 

`"foo" + "bar"`

has the value `"foobar"`

The `+` operator can also be used to append two lists. The expression

```
{1, 2, 3} + {4, 5, 6}
```

has the value `{1, 2, 3, 4, 5, 6}`

The `+` operator can also be used to append to a list. The expression

```
{1, 2} + #123
```

has the value of `{1, 2, #123}`
Unless both operands to an arithmetic operator are numbers of the same kind (or, for `+`, both strings), the error value `E_TYPE` is raised. If the right-hand operand for the division or remainder operators (`/` or `%`) is zero, the error value `E_DIV` is raised.

MOO also supports the exponentiation operation, also known as "raising to a power," using the `^` operator:

```
3 ^ 4       =>   81
3 ^ 4.5     error-->   E_TYPE
3.5 ^ 4     =>   150.0625
3.5 ^ 4.5   =>   280.741230801382
```

> Note: if the first operand is an integer, then the second operand must also be an integer. If the first operand is a floating-point number, then the second operand can be either kind of number. Although it is legal to raise an integer to a negative power, it is unlikely to be terribly useful.

#### Bitwise Operators

MOO also supports bitwise operations on integer types: 

| Operator | Meaning                              |
| -------- | ------------------------------------ |
| &.       | bitwise `and`                        |
| \|.      | bitwise `or`                         |
| ^.       | bitwise `xor`                        |
| >>       | logical (not arithmetic) right-shift |
| <<       | logical (not arithmetic) left-shift  |
| ~        | complement                           |

In the following table, the expressions on the left have the corresponding values on the right:

```
1 &. 2       =>  0
1 |. 2       =>  3
1 ^. 3       =>  1
8 << 1       =>  16
8 >> 1       =>  4
~0           =>  -1
```

For more information on Bitwise Operators, checkout the [Wikipedia](https://en.wikipedia.org/wiki/Bitwise_operation) page on them.

#### Comparing Values

Any two values can be compared for equality using `==` and `!=`. The first of these returns 1 if the two values are equal and 0 otherwise; the second does the reverse:

```
3 == 4                              =>  0
3 != 4                              =>  1
3 == 3.0                            =>  0
"foo" == "Foo"                      =>  1
#34 != #34                          =>  0
{1, #34, "foo"} == {1, #34, "FoO"}  =>  1
E_DIV == E_TYPE                     =>  0
3 != "foo"                          =>  1
[1 -> 2] == [1 -> 2]                =>  1
[1 -> 2] == [2 -> 1]                =>  0
true == true                        =>  1
false == true                       =>  0
```

Note that integers and floating-point numbers are never equal to one another, even in the _obvious_ cases. Also note that comparison of strings (and list values containing strings) is case-insensitive; that is, it does not distinguish between the upper- and lower-case version of letters. To test two values for case-sensitive equality, use the `equal` function described later.

> Warning: It is easy (and very annoying) to confuse the equality-testing operator (`==`) with the assignment operator (`=`), leading to nasty, hard-to-find bugs. Don't do this.

> Warning: Comparing floating point numbers for equality can be tricky. Sometimes two floating point numbers will appear the same but be rounded up or down at some meaningful bit, and thus will not be exactly equal. This is especially true when comparing a number in memory (assigned to a variable) to a number that is formed from reading a value from a player, or pulled from a property. Be wary of this, if you ever encounter it, as it can be tedious to debug.

Integers, floats, object numbers, strings, and error values can also be compared for ordering purposes using the following operators:

| Operator | Meaning                           |
| -------- | --------------------------------- |
| &lt;     | meaning &quot;less than&quot;     |
| &lt;=    | &quot;less than or equal&quot;    |
| &gt;=    | &quot;greater than or equal&quot; |
| &gt;     | &quot;greater than&quot;          |

As with the equality operators, these return 1 when their operands are in the appropriate relation and 0 otherwise:

```
3 < 4           =>  1
3 < 4.0         =>  E_TYPE (an error)
#34 >= #32      =>  1
"foo" <= "Boo"  =>  0
E_DIV > E_TYPE  =>  1
```

Note that, as with the equality operators, strings are compared case-insensitively. To perform a case-sensitive string comparison, use the `strcmp` function described later. Also note that the error values are ordered as given in the table in the section on values. If the operands to these four comparison operators are of different types (even integers and floating-point numbers are considered different types), or if they are lists, then `E_TYPE` is raised.

#### Values as True and False

There is a notion in MOO of _true_ and _false_ values; every value is one or the other. The true values are as follows:

* all integers other than zero (positive or negative)
* all floating-point numbers not equal to `0.0`
* all non-empty strings (i.e., other than `""`)
* all non-empty lists (i.e., other than `{}`)
* all non-empty maps (i.e, other than `[]`)
* the bool 'true'

All other values are false:

* the integer zero
* the floating-point numbers `0.0` and `-0.0`
* the empty string (`""`)
* the empty list (`{}`)
* all object numbers & object references 
* all error values
* the bool 'false'

> Note: Objects are considered false. If you need to evaluate if a value is of the type object, you can use `typeof(potential_object) == OBJ` however, keep in mind that this does not mean that the object referenced actually exists. IE: #100000000 will return true, but that does not mean that object exists in your MOO.

> Note: Don't get confused between values evaluating to true or false, and the boolean values of `true` and `false`.

There are four kinds of expressions and two kinds of statements that depend upon this classification of MOO values. In describing them, I sometimes refer to the _truth value_ of a MOO value; this is just _true_ or _false_, the category into which that MOO value is classified.

The conditional expression in MOO has the following form:

```
expression-1 ? expression-2 | expression-3
```

> Note: This is commonly referred to as a ternary statement in most programming languages. In MOO the commonly used ! is replaced with a |.

First, expression-1 is evaluated. If it returns a true value, then expression-2 is evaluated and whatever it returns is returned as the value of the conditional expression as a whole. If expression-1 returns a false value, then expression-3 is evaluated instead and its value is used as that of the conditional expression.

```
1 ? 2 | 3           =>  2
0 ? 2 | 3           =>  3
"foo" ? 17 | {#34}  =>  17
```

Note that only one of expression-2 and expression-3 is evaluated, never both.

To negate the truth value of a MOO value, use the `!` operator:

```
! expression
```

If the value of expression is true, `!` returns 0; otherwise, it returns 1:

```
! "foo"     =>  0
! (3 >= 4)  =>  1
```

> Note: The "negation" or "not" operator is commonly referred to as "bang" in modern parlance.

It is frequently useful to test more than one condition to see if some or all of them are true. MOO provides two operators for this:

```
expression-1 && expression-2
expression-1 || expression-2
```

These operators are usually read as "and" and "or," respectively.

The `&&` operator first evaluates expression-1. If it returns a true value, then expression-2 is evaluated and its value becomes the value of the `&&` expression as a whole; otherwise, the value of expression-1 is used as the value of the `&&` expression.

> Note: expression-2 is only evaluated if expression-1 returns a true value.

The `&&` expression is equivalent to the conditional expression:

```
expression-1 ? expression-2 | expression-1
```

except that expression-1 is only evaluated once.

The `||` operator works similarly, except that expression-2 is evaluated only if expression-1 returns a false value. It is equivalent to the conditional expression:

```
expression-1 ? expression-1 | expression-2
```

except that, as with `&&`, expression-1 is only evaluated once.

These two operators behave very much like "and" and "or" in English:

```
1 && 1                  =>  1
0 && 1                  =>  0
0 && 0                  =>  0
1 || 1                  =>  1
0 || 1                  =>  1
0 || 0                  =>  0
17 <= 23  &&  23 <= 27  =>  1
```

#### Indexing into Lists, Maps and Strings

Strings, lists, and maps can be seen as ordered sequences of MOO values. In the case of strings, each is a sequence of single-character strings; that is, one can view the string `"bar"` as a sequence of the strings `"b"`, `"a"`, and `"r"`. MOO allows you to refer to the elements of lists, maps, and strings by number, by the _index_ of that element in the list or string. The first element has index 1, the second has index 2, and so on.

> Warning: It is very important to note that unlike many programming languages (which use 0 as the starting index), MOO uses 1.

##### Extracting an Element by Index

The indexing expression in MOO extracts a specified element from a list, map, or string:

```
expression-1[expression-2]
```

First, expression-1 is evaluated; it must return a list, map, or string (the _sequence_). Then, expression-2 is evaluated and must return an integer (the _index_) or the _key_ in the case of maps. If either of the expressions returns some other type of value, `E_TYPE` is returned.

For lists and strings the index must be between 1 and the length of the sequence, inclusive; if it is not, then `E_RANGE` is raised.  The value of the indexing expression is the index'th element in the sequence. For maps, the key must be present, if it is not, then E_RANGE is raised. Within expression-2 you can use the symbol ^ as an expression returning the index or key of the first element in the sequence and you can use the symbol $ as an expression returning the index or key of the last element in expression-1.

```
"fob"[2]                =>  "o"
[1 -> "A"][1]           =>  "A"
"fob"[1]                =>  "f"
{#12, #23, #34}[$ - 1]  =>  #23
```

Note that there are no legal indices for the empty string or list, since there are no integers between 1 and 0 (the length of the empty string or list).

Fine point: The ^ and $ expressions return the first/last index/key of the expression just before the nearest enclosing [...] indexing or subranging brackets. For example:

```
"frob"[{3, 2, 4}[^]]     =>  "o"
"frob"[{3, 2, 4}[$]]     =>  "b"
```

is possible because $ in this case represents the 3rd index of the list next to it, which evaluates to the value 4, which in turn is applied as the index to the string, which evaluates to the b.

##### Replacing an Element of a List, Map, or String

It often happens that one wants to change just one particular slot of a list or string, which is stored in a variable or a property. This can be done conveniently using an _indexed assignment_ having one of the following forms:

```
variable[index-expr] = result-expr
object-expr.name[index-expr] = result-expr
object-expr.(name-expr)[index-expr] = result-expr
$name[index-expr] = result-expr
```

The first form writes into a variable, and the last three forms write into a property. The usual errors (`E_TYPE`, `E_INVIND`, `E_PROPNF` and `E_PERM` for lack of read/write permission on the property) may be raised, just as in reading and writing any object property; see the discussion of object property expressions below for details.

Correspondingly, if variable does not yet have a value (i.e., it has never been assigned to), `E_VARNF` will be raised.

If index-expr is not an integer (for lists and strings) or is a collection value (for maps), or if the value of `variable` or the property is not a list, map or string, `E_TYPE` is raised. If `result-expr` is a string, but not of length 1, E_INVARG is raised. Suppose `index-expr` evaluates to a value `k`. If `k` is an integer and is outside the range of the list or string (i.e. smaller than 1 or greater than the length of the list or string), `E_RANGE` is raised. If `k` is not a valid key of the map, `E_RANGE` is raised. Otherwise, the actual assignment takes place. 

For lists, the variable or the property is assigned a new list that is identical to the original one except at the k-th position, where the new list contains the result of result-expr instead. Likewise for maps, the variable or the property is assigned a new map that is identical to the original one except for the k key, where the new map contains the result of result-expr instead. For strings, the variable or the property is assigned a new string that is identical to the original one, except the k-th character is changed to be result-expr.

If index-expr is not an integer, or if the value of variable or the property is not a list or string, `E_TYPE` is raised. If result-expr is a string, but not of length 1, `E_INVARG` is raised. Now suppose index-expr evaluates to an integer n. If n is outside the range of the list or string (i.e. smaller than 1 or greater than the length of the list or string), `E_RANGE` is raised. Otherwise, the actual assignment takes place.

For lists, the variable or the property is assigned a new list that is identical to the original one except at the n-th position, where the new list contains the result of result-expr instead. For strings, the variable or the property is assigned a new string that is identical to the original one, except the n-th character is changed to be result-expr.

The assignment expression itself returns the value of result-expr. For the following examples, assume that `l` initially contains the list `{1, 2, 3}`, that `m` initially contains the map `["one" -> 1, "two" -> 2]` and that `s` initially contains the string "foobar": 

```
l[5] = 3          =>   E_RANGE (error)
l["first"] = 4    =>   E_TYPE  (error)
s[3] = "baz"      =>   E_INVARG (error)
l[2] = l[2] + 3   =>   5
l                 =>   {1, 5, 3}
l[2] = "foo"      =>   "foo"
l                 =>   {1, "foo", 3}
s[2] = "u"        =>   "u"
s                 =>   "fuobar"
s[$] = "z"        =>   "z"
s                 =>   "fuobaz"
m                 =>   ["foo" -> "bar"]
m[1] = "baz"      =>   ["foo" -> "baz"]
```

> Note: (error) is only used for formatting and identification purposes in these examples and is not present in an actual raised error on the MOO.

> Note: The `$` expression may also be used in indexed assignments with the same meaning as before.

Fine point: After an indexed assignment, the variable or property contains a _new_ list or string, a copy of the original list in all but the n-th place, where it contains a new value. In programming-language jargon, the original list is not mutated, and there is no aliasing. (Indeed, no MOO value is mutable and no aliasing ever occurs.)

In the list and map case, indexed assignment can be nested to many levels, to work on nested lists and maps. Assume that `l` initially contains the following 

```
{{1, 2, 3}, {4, 5, 6}, "foo", ["bar" -> "baz"]}
```

in the following examples:

```
l[7] = 4             =>   E_RANGE (error)
l[1][8] = 35         =>   E_RANGE (error)
l[3][2] = 7          =>   E_TYPE (error)
l[1][1][1] = 3       =>   E_TYPE (error)
l[2][2] = -l[2][2]   =>   -5
l                    =>   {{1, 2, 3}, {4, -5, 6}, "foo", ["bar" -> "baz"]}
l[2] = "bar"         =>   "bar"
l                    =>   {{1, 2, 3}, "bar", "foo", ["bar" -> "baz"]}
l[2][$] = "z"        =>   "z"
l                    =>   {{1, 2, 3}, "baz", "foo", ["bar" -> "baz"]}
l[$][^] = #3         =>   #3
l                    =>   {{1, 2, 3}, "baz", "foo", ["bar" -> #3]}
```

The first two examples raise E_RANGE because 7 is out of the range of `l` and 8 is out of the range of `l[1]`. The next two examples raise `E_TYPE` because `l[3]` and `l[1][1]` are not lists. 

##### Extracting a Subsequence of a List, Map or String
The range expression extracts a specified subsequence from a list, map or string:

```
expression-1[expression-2..expression-3]
```

The three expressions are evaluated in order. Expression-1 must return a list, map or string (the _sequence_) and the other two expressions must return integers (the _low_ and _high_ indices, respectively) for lists and strings, or non-collection values (the `begin` and `end` keys in the ordered map, respectively) for maps; otherwise, `E_TYPE` is raised. The `^` and `$` expression can be used in either or both of expression-2 and expression-3 just as before.

If the low index is greater than the high index, then the empty string, list or map is returned, depending on whether the sequence is a string, list or map.  Otherwise, both indices must be between 1 and the length of the sequence (for lists or strings) or valid keys (for maps); `E_RANGE` is raised if they are not. A new list, map or string is returned that contains just the elements of the sequence with indices between the low/high and high/end bounds.

```
"foobar"[2..$]                       =>  "oobar"
"foobar"[3..3]                       =>  "o"
"foobar"[17..12]                     =>  ""
{"one", "two", "three"}[$ - 1..$]    =>  {"two", "three"}
{"one", "two", "three"}[3..3]        =>  {"three"}
{"one", "two", "three"}[17..12]      =>  {}
[1 -> "one", 2 -> "two"][1..1]       =>  [1 -> "one"]
```

##### Replacing a Subsequence of a List, Map or String

The subrange assignment replaces a specified subsequence of a list, map or string with a supplied subsequence. The allowed forms are:

```
variable[start-index-expr..end-index-expr] = result-expr
object-expr.name[start-index-expr..end-index-expr] = result-expr
object-expr.(name-expr)[start-index-expr..end-index-expr] = result-expr
$name[start-index-expr..end-index-expr] = result-expr
```

As with indexed assignments, the first form writes into a variable, and the last three forms write into a property. The same errors (`E_TYPE`, `E_INVIND`, `E_PROPNF` and `E_PERM` for lack of read/write permission on the property) may be raised. If variable does not yet have a value (i.e., it has never been assigned to), `E_VARNF` will be raised. As before, the `^` and `$` expression can be used in either start-index-expr or end-index-expr.

If start-index-expr or end-index-expr is not an integer (for lists and strings) or a collection value (for maps), if the value of variable or the property is not a list, map, or string, or result-expr is not the same type as variable or the property, `E_TYPE` is raised. For lists and strings, `E_RANGE` is raised if end-index-expr is less than zero or if start-index-expr is greater than the length of the list or string plus one. Note: the length of result-expr does not need to be the same as the length of the specified range. For maps, `E_RANGE` is raised if `start-index-expr` or `end-index-expr` are not keys in the map.

In precise terms, the subrange assignment 

```
v[start..end] = value
```

is equivalent to

```
v = {@v[1..start - 1], @value, @v[end + 1..$]}
```

if v is a list and to

```
v = v[1..start - 1] + value + v[end + 1..$]
```

if v is a string. 

There is no literal representation of the operation if v is a map. In this case the range given by start-index-expr and end-index-expr is removed, and the the values in result-expr are added.

The assignment expression itself returns the value of result-expr.

> Note: The use of preceding a list with the @ symbol is covered in just a bit.

For the following examples, assume that `l` initially contains the list `{1, 2, 3}`, that `m` initially contains the map [1 -> "one", 2 -> "two", 3 -> "three"] and that `s` initially contains the string "foobar":

```
l[5..6] = {7, 8}       =>   E_RANGE (error)
l[2..3] = 4            =>   E_TYPE (error)
l[#2..3] = {7}         =>   E_TYPE (error)
s[2..3] = {6}          =>   E_TYPE (error)
l[2..3] = {6, 7, 8, 9} =>   {6, 7, 8, 9}
l                      =>   {1, 6, 7, 8, 9}
l[2..1] = {10, "foo"}  =>   {10, "foo"}
l                      =>   {1, 10, "foo", 6, 7, 8, 9}
l[3][2..$] = "u"       =>   "u"
l                      =>   {1, 10, "fu", 6, 7, 8, 9}
s[7..12] = "baz"       =>   "baz"
s                      =>   "foobarbaz"
s[1..3] = "fu"         =>   "fu"
s                      =>   "fubarbaz"
s[1..0] = "test"       =>   "test"
s                      =>   "testfubarbaz"
m[1..2] = ["abc" -> #1]=>   ["abc" -> #1]
m                      =>   [3 -> "three", "abc" -> #1]
```

#### Other Operations on Lists

As was mentioned earlier, lists can be constructed by writing a comma-separated sequence of expressions inside curly braces:

```
{expression-1, expression-2, ..., expression-N}
```

The resulting list has the value of expression-1 as its first element, that of expression-2 as the second, etc.

```
{3 < 4, 3 <= 4, 3 >= 4, 3 > 4}  =>  {1, 1, 0, 0}
```

The addition operator works with lists. When adding two lists together, the two will be concatenated:

```
{1, 2, 3} + {4, 5, 6} => {1, 2, 3, 4, 5, 6}) 
```

When adding another type to a list, it will append that value to the end of the list:

```
{1, 2} + #123 => {1, 2, #123}
```

Additionally, one may precede any of these expressions by the splicing operator, `@`. Such an expression must return a list; rather than the old list itself becoming an element of the new list, all of the elements of the old list are included in the new list. This concept is easy to understand, but hard to explain in words, so here are some examples. For these examples, assume that the variable `a` has the value `{2, 3, 4}` and that `b` has the value `{"Foo", "Bar"}`:

```
{1, a, 5}   =>  {1, {2, 3, 4}, 5}
{1, @a, 5}  =>  {1, 2, 3, 4, 5}
{a, @a}     =>  {{2, 3, 4}, 2, 3, 4}
{@a, @b}    =>  {2, 3, 4, "Foo", "Bar"}
```

If the splicing operator (`@`) precedes an expression whose value is not a list, then `E_TYPE` is raised as the value of the list construction as a whole.

The list membership expression tests whether or not a given MOO value is an element of a given list and, if so, with what index:

```
expression-1 in expression-2
```

Expression-2 must return a list; otherwise, `E_TYPE` is raised.  If the value of expression-1 is in that list, then the index of its first occurrence in the list is returned; otherwise, the `in` expression returns 0.

```
2 in {5, 8, 2, 3}               =>  3
7 in {5, 8, 2, 3}               =>  0
"bar" in {"Foo", "Bar", "Baz"}  =>  2
```

Note that the list membership operator is case-insensitive in comparing strings, just like the comparison operators. To perform a case-sensitive list membership test, use the `is_member` function described later. Note also that since it returns zero only if the given value is not in the given list, the `in` expression can be used either as a membership test or as an element locator.

#### Spreading List Elements Among Variables

It is often the case in MOO programming that you will want to access the elements of a list individually, with each element stored in a separate variables. This desire arises, for example, at the beginning of almost every MOO verb, since the arguments to all verbs are delivered all bunched together in a single list. In such circumstances, you _could_ write statements like these:

```
first = args[1];
second = args[2];
if (length(args) > 2)
  third = args[3];
else
  third = 0;
endif
```

This approach gets pretty tedious, both to read and to write, and it's prone to errors if you mistype one of the indices. Also, you often want to check whether or not any _extra_ list elements were present, adding to the tedium.

MOO provides a special kind of assignment expression, called _scattering assignment_ made just for cases such as these. A scattering assignment expression looks like this:

```
{target, ...} = expr
```

where each target describes a place to store elements of the list that results from evaluating expr. A target has one of the following forms:

| Target                                | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <code>variable</code>                 | This is the simplest target, just a simple variable; the list element in the corresponding position is assigned to the variable. This is called a <em>required</em> target, since the assignment is required to put one of the list elements into the variable.                                                                                                                                                                                                                                                                                                |
| <code>?variable</code>                | This is called an <em>optional</em> target, since it doesn't always get assigned an element. If there are any list elements left over after all of the required targets have been accounted for (along with all of the other optionals to the left of this one), then this variable is treated like a required one and the list element in the corresponding position is assigned to the variable. If there aren't enough elements to assign one to this target, then no assignment is made to this variable, leaving it with whatever its previous value was. |
| <code>?variable = default-expr</code> | This is also an optional target, but if there aren't enough list elements available to assign one to this target, the result of evaluating default-expr is assigned to it instead. Thus, default-expr provides a <em>default value</em> for the variable. The default value expressions are evaluated and assigned working from left to right <em>after</em> all of the other assignments have been performed.                                                                                                                                                 |
| <code>@variable</code>                | By analogy with the <code>@</code> syntax in list construction, this variable is assigned a list of all of the 'leftover' list elements in this part of the list after all of the other targets have been filled in. It is assigned the empty list if there aren't any elements left over. This is called a <em>rest</em> target, since it gets the rest of the elements. There may be at most one rest target in each scattering assignment expression.                                                                                                       |

If there aren't enough list elements to fill all of the required targets, or if there are more than enough to fill all of the required and optional targets but there isn't a rest target to take the leftover ones, then `E_ARGS` is raised.

Here are some examples of how this works. Assume first that the verb `me:foo()` contains the following code:

```
b = c = e = 17;
{a, ?b, ?c = 8, @d, ?e = 9, f} = args;
return {a, b, c, d, e, f};
```

Then the following calls return the given values:

```
me:foo(1)                        =>   E_ARGS (error)
me:foo(1, 2)                     =>   {1, 17, 8, {}, 9, 2}
me:foo(1, 2, 3)                  =>   {1, 2, 8, {}, 9, 3}
me:foo(1, 2, 3, 4)               =>   {1, 2, 3, {}, 9, 4}
me:foo(1, 2, 3, 4, 5)            =>   {1, 2, 3, {}, 4, 5}
me:foo(1, 2, 3, 4, 5, 6)         =>   {1, 2, 3, {4}, 5, 6}
me:foo(1, 2, 3, 4, 5, 6, 7)      =>   {1, 2, 3, {4, 5}, 6, 7}
me:foo(1, 2, 3, 4, 5, 6, 7, 8)   =>   {1, 2, 3, {4, 5, 6}, 7, 8}
```

Using scattering assignment, the example at the beginning of this section could be rewritten more simply, reliably, and readably:

```
{first, second, ?third = 0} = args;
```

Fine point: If you are familiar with JavaScript, the 'rest' and 'spread' functionality should look pretty familiar. It is good MOO programming style to use a scattering assignment at the top of nearly every verb (at least ones that are 'this none this'), since it shows so clearly just what kinds of arguments the verb expects.

#### Operations on BOOLs

ToastStunt offers a `bool` type. This type can be either `true` which is considered `1` or `false` which is considered `0`. Boolean values can be set in your code/props much the same way any other value can be assigned to a variable or property.

```
;true                   => true
;false                  => false
;true == true           => 1
;false == false         => 1
;true == false          => 0
;1 == true              => 1
;5 == true              => 0
;0 == false             => 1
;-1 == false            => 0
!true                   => 0
!false                  => 1
!false == true          => 1
!true == false          => 1
```

The true and false variables are set at task runtime (or your code) and can be overridden within verbs if needed. This will not carryover after the verb is finished executing.

> Fine Point: As mentioned earlier, there are constants like STR which resolved to the integer code 2. OBJ resolves to the integer code of 1. Thus if you were to execute code such as `typeof(#15840) == TRUE` you would get a truthy response, as typeof() would return `1` to denote the object's integer code. This is a side effect of `true` always equaling 1, for compatibility reasons.

#### Getting and Setting the Values of Properties

Usually, one can read the value of a property on an object with a simple expression:

```
expression.name
```

Expression must return an object number; if not, `E_TYPE` is raised. If the object with that number does not exist, `E_INVIND` is raised. Otherwise, if the object does not have a property with that name, then `E_PROPNF` is raised. Otherwise, if the named property is not readable by the owner of the current verb, then `E_PERM` is raised.  Finally, assuming that none of these terrible things happens, the value of the named property on the given object is returned.

I said "usually" in the paragraph above because that simple expression only works if the name of the property obeys the same rules as for the names of variables (i.e., consists entirely of letters, digits, and underscores, and doesn't begin with a digit). Property names are not restricted to this set, though. Also, it is sometimes useful to be able to figure out what property to read by some computation. For these more general uses, the following syntax is also allowed:

```
expression-1.(expression-2)
```

As before, expression-1 must return an object number. Expression-2 must return a string, the name of the property to be read; `E_TYPE` is raised otherwise. Using this syntax, any property can be read, regardless of its name.

Note that, as with almost everything in MOO, case is not significant in the names of properties. Thus, the following expressions are all equivalent:

```
foo.bar
foo.Bar
foo.("bAr")
```

The ToastCore database uses several properties on `#0`, the _system object_, for various special purposes. For example, the value of `#0.room` is the "generic room" object, `#0.exit` is the "generic exit" object, etc. This allows MOO programs to refer to these useful objects more easily (and more readably) than using their object numbers directly. To make this usage even easier and more readable, the expression

```
$name
```

(where name obeys the rules for variable names) is an abbreviation for

```
#0.name
```

Thus, for example, the value `$nothing` mentioned earlier is really `#-1`, the value of `#0.nothing`.

As with variables, one uses the assignment operator (`=`) to change the value of a property. For example, the expression

```
14 + (#27.foo = 17)
```

changes the value of the `foo` property of the object numbered 27 to be 17 and then returns 31. Assignments to properties check that the owner of the current verb has write permission on the given property, raising `E_PERM` otherwise. Read permission is not required.

#### Calling Built-in Functions and Other Verbs

MOO provides a large number of useful functions for performing a wide variety of operations; a complete list, giving their names, arguments, and semantics, appears in a separate section later. As an example to give you the idea, there is a function named `length` that returns the length of a given string or list.

The syntax of a call to a function is as follows:

```
name(expr-1, expr-2, ..., expr-N)
```

where name is the name of one of the built-in functions. The expressions between the parentheses, called _arguments_, are each evaluated in turn and then given to the named function to use in its appropriate way. Most functions require that a specific number of arguments be given; otherwise, `E_ARGS` is raised. Most also require that certain of the arguments have certain specified types (e.g., the `length()` function requires a list or a string as its argument); `E_TYPE` is raised if any argument has the wrong type.

As with list construction, the splicing operator `@` can precede any argument expression. The value of such an expression must be a list; `E_TYPE` is raised otherwise. The elements of this list are passed as individual arguments, in place of the list as a whole.

Verbs can also call other verbs, usually using this syntax:

```
expr-0:name(expr-1, expr-2, ..., expr-N)
```

Expr-0 must return an object number; `E_TYPE` is raised otherwise.  If the object with that number does not exist, `E_INVIND` is raised. If this task is too deeply nested in verbs calling verbs calling verbs, then `E_MAXREC` is raised; the default limit is 50 levels, but this can be changed from within the database; see the chapter on server assumptions about the database for details. If neither the object nor any of its ancestors defines a verb matching the given name, `E_VERBNF` is raised.  Otherwise, if none of these nasty things happens, the named verb on the given object is called; the various built-in variables have the following initial values in the called verb:

| Variable            | Description                                                                                                                                                                      |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <code>this</code>   | an object, the value of expr-0                                                                                                                                                   |
| <code>verb</code>   | a string, the name used in calling this verb                                                                                                                                     |
| <code>args</code>   | a list, the values of expr-1, expr-2, etc.                                                                                                                                       |
| <code>caller</code> | an object, the value of <code>this</code> in the calling verb                                                                                                                    |
| <code>player</code> | an object, the same value as it had initially in the calling verb or, if the calling verb is running with wizard permissions, the same as the current value in the calling verb. |

All other built-in variables (`argstr`, `dobj`, etc.) are initialized with the same values they have in the calling verb.

As with the discussion of property references above, I said "usually" at the beginning of the previous paragraph because that syntax is only allowed when the name follows the rules for allowed variable names. Also as with property reference, there is a syntax allowing you to compute the name of the verb:

```
expr-0:(expr-00)(expr-1, expr-2, ..., expr-N)
```

The expression expr-00 must return a string; `E_TYPE` is raised otherwise.

The splicing operator (`@`) can be used with verb-call arguments, too, just as with the arguments to built-in functions.

In many databases, a number of important verbs are defined on `#0`, the _system object_. As with the `$foo` notation for properties on `#0`, the server defines a special syntax for calling verbs on `#0`:

```
$name(expr-1, expr-2, ..., expr-N)
```

(where name obeys the rules for variable names) is an abbreviation for

```
#0:name(expr-1, expr-2, ..., expr-N)
```

#### Verb Calls on Primitive Types

The server supports verbs calls on primitive types (numbers, strings, etc.) so calls like `"foo bar":split()` can be implemented and work as expected (they were always syntactically correct in LambdaMOO but resulted in an E_TYPE error).  Verbs are implemented on prototype object delegates ($int_proto, $float_proto, $str_proto, etc.).  The server transparently invokes the correct verb on the appropriate prototype -- the primitive value is the value of `this'.

This also includes supporting calling verbs on an object prototype ($obj_proto). Counterintuitively, this will only work for types of OBJ that are invalid. This can come in useful for un-logged-in connections (i.e. creating a set of convenient utilities for dealing with negative connections in-MOO).

> Fine Point: Utilizing verbs on primitives is a matter of style. Some people like it, some people don't. The author suggests you keep a utility object (like $string_utils) and simply forward verb calls from your primitive to this utility, which keeps backwards compatibility with how ToastCore and LambdaCore are generally built. By default in ToastCore, the primitives just wrap around their `type`_utils counterparts.

#### Catching Errors in Expressions

It is often useful to be able to _catch_ an error that an expression raises, to keep the error from aborting the whole task, and to keep on running as if the expression had returned some other value normally. The following expression accomplishes this:

```
` expr-1 ! codes => expr-2 '
```

> Note: The open- and close-quotation marks in the previous line are really part of the syntax; you must actually type them as part of your MOO program for this kind of expression.

The codes part is either the keyword `ANY` or else a comma-separated list of expressions, just like an argument list. As in an argument list, the splicing operator (`@`) can be used here. The `=> expr-2` part of the error-catching expression is optional.

First, the codes part is evaluated, yielding a list of error codes that should be caught if they're raised; if codes is `ANY`, then it is equivalent to the list of all possible MOO values.

Next, expr-1 is evaluated. If it evaluates normally, without raising an error, then its value becomes the value of the entire error-catching expression. If evaluating expr-1 results in an error being raised, then call that error E. If E is in the list resulting from evaluating codes, then E is considered _caught_ by this error-catching expression. In such a case, if expr-2 was given, it is evaluated to get the outcome of the entire error-catching expression; if expr-2 was omitted, then E becomes the value of the entire expression. If E is _not_ in the list resulting from codes, then this expression does not catch the error at all and it continues to be raised, possibly to be caught by some piece of code either surrounding this expression or higher up on the verb-call stack.

Here are some examples of the use of this kind of expression:

```
`x + 1 ! E_TYPE => 0'
```

Returns `x + 1` if `x` is an integer, returns `0` if `x` is not an integer, and raises `E_VARNF` if `x` doesn't have a value.

```
`x.y ! E_PROPNF, E_PERM => 17'
```

Returns `x.y` if that doesn't cause an error, `17` if `x` doesn't have a `y` property or that property isn't readable, and raises some other kind of error (like `E_INVIND`) if `x.y` does.

```
`1 / 0 ! ANY'
```

Returns `E_DIV`.

> Note: It's important to mention how powerful this compact syntax for writing error catching code can be.  When used properly you can write very complex and elegant code. For example imagine that you have a set of objects from different parents, some of which define a specific verb, and some of which do not. If for instance, your code wants to perform some function _if_ the verb exists, you can write `obj:verbname() ! E_VERBNF' to allow the MOO to attempt to execute that verb and then if it fails, catch the error and continue operations normally.

#### Parentheses and Operator Precedence

As shown in a few examples above, MOO allows you to use parentheses to make it clear how you intend for complex expressions to be grouped. For example, the expression

```
3 * (4 + 5)
```

performs the addition of 4 and 5 before multiplying the result by 3.

If you leave out the parentheses, MOO will figure out how to group the expression according to certain rules. The first of these is that some operators have higher _precedence_ than others; operators with higher precedence will more tightly bind to their operands than those with lower precedence. For example, multiplication has higher precedence than addition; thus, if the parentheses had been left out of the expression in the previous paragraph, MOO would have grouped it as follows:

```
(3 * 4) + 5
```

The table below gives the relative precedence of all of the MOO operators; operators on higher lines in the table have higher precedence and those on the same line have identical precedence:

```
!       - (without a left operand)
^
*       /       %
+       -
==      !=      <       <=      >       >=      in
&&      ||
... ? ... | ... (the conditional expression)
=
```

Thus, the horrendous expression

```
x = a < b && c > d + e * f ? w in y | - q - r
```

would be grouped as follows:

```
x = (((a < b) && (c > (d + (e * f)))) ? (w in y) | ((- q) - r))
```

It is best to keep expressions simpler than this and to use parentheses liberally to make your meaning clear to other humans.

### MOO Language Statements

Statements are MOO constructs that, in contrast to expressions, perform some useful, non-value-producing operation. For example, there are several kinds of statements, called _looping constructs_, that repeatedly perform some set of operations. Fortunately, there are many fewer kinds of statements in MOO than there are kinds of expressions.

#### Errors While Executing Statements

Statements do not return values, but some kinds of statements can, under certain circumstances described below, generate errors. If such an error is generated in a verb whose `d` (debug) bit is not set, then the error is ignored and the statement that generated it is simply skipped; execution proceeds with the next statement.

> Note: This error-ignoring behavior is very error prone, since it affects _all_ errors, including ones the programmer may not have anticipated. The `d` bit exists only for historical reasons; it was once the only way for MOO programmers to catch and handle errors. The error-catching expression and the `try` -`except` statement are far better ways of accomplishing the same thing.

If the `d` bit is set, as it usually is, then the error is _raised_ and can be caught and handled either by code surrounding the expression in question or by verbs higher up on the chain of calls leading to the current verb. If the error is not caught, then the server aborts the entire task and, by default, prints a message to the current player. See the descriptions of the error-catching expression and the `try`-`except` statement for the details of how errors can be caught, and the chapter on server assumptions about the database for details on the handling of uncaught errors.

#### Simple Statements

The simplest kind of statement is the _null_ statement, consisting of just a semicolon:

```
;
```

It doesn't do anything at all, but it does it very quickly.

The next simplest statement is also one of the most common, the expression statement, consisting of any expression followed by a semicolon:

```
expression;
```

The given expression is evaluated and the resulting value is ignored.  Commonly-used kinds of expressions for such statements include assignments and verb calls. Of course, there's no use for such a statement unless the evaluation of expression has some side-effect, such as changing the value of some variable or property, printing some text on someone's screen, etc.

```
#42.weight = 40;
#42.weight;
2 + 5;
obj:verbname();
1 > 2;
2 < 1;
```

#### Statements for Testing Conditions

The `if` statement allows you to decide whether or not to perform some statements based on the value of an arbitrary expression:

```
if (expression)
  statements
endif
```

Expression is evaluated and, if it returns a true value, the statements are executed in order; otherwise, nothing more is done.

One frequently wants to perform one set of statements if some condition is true and some other set of statements otherwise. The optional `else` phrase in an `if` statement allows you to do this:

```
if (expression)
  statements-1
else
  statements-2
endif
```

This statement is executed just like the previous one, except that statements-1 are executed if expression returns a true value and statements-2 are executed otherwise.

Sometimes, one needs to test several conditions in a kind of nested fashion:

```
if (expression-1)
  statements-1
else
  if (expression-2)
    statements-2
  else
    if (expression-3)
      statements-3
    else
      statements-4
    endif
  endif
endif
```

Such code can easily become tedious to write and difficult to read. MOO provides a somewhat simpler notation for such cases:

```
if (expression-1)
  statements-1
elseif (expression-2)
  statements-2
elseif (expression-3)
  statements-3
else
  statements-4
endif
```

Note that `elseif` is written as a single word, without any spaces. This simpler version has the very same meaning as the original: evaluate expression-i for i equal to 1, 2, and 3, in turn, until one of them returns a true value; then execute the statements-i associated with that expression. If none of the expression-i return a true value, then execute statements-4.

Any number of `elseif` phrases can appear, each having this form:

```
elseif (expression)
    statements
```

The complete syntax of the `if` statement, therefore, is as follows:

```
if (expression)
  statements
zero-or-more-elseif-phrases
an-optional-else-phrase
endif
```

#### Statements for Looping

MOO provides three different kinds of looping statements, allowing you to have a set of statements executed (1) once for each element of a given sequence (list, map or string); (2) once for each integer or object number in a given range; and (3) over and over until a given condition stops being true.

To perform some statements once for each element of a given sequence, use this syntax:
 	
```
for value, key-or-index in (expression)
  statements
endfor
```

The expression is evaluated and should return a list, map or string; if it does not, E_TYPE is raised. The statements are then executed once for each element of that sequence in turn; each time, the given value is assigned the value of the element in question, and key-or-index is assigned the index of value in the list or string, or its key if the sequence is a map. key-or-index is optional. For example, consider the following statements:
 	
```
odds = {1, 3, 5, 7, 9};
evens = {};
for n in (odds)
  evens = {@evens, n + 1};
endfor
```

The value of the variable `evens` after executing these statements is the list
 	

`{2, 4, 6, 8, 10}`

If the example were modified:

```
odds = {1, 3, 5, 7, 9};
pairs = [];
for n, i in (odds)
  pairs[i] = n + 1;
endfor
```

The value of the variable `pairs` after executing these statements is the map
 	

`[1 -> 2, 2 -> 4, 3 -> 6, 4 -> 8, 5 -> 10]`

To perform a set of statements once for each integer or object number in a given range, use this syntax:

```
for variable in [expression-1..expression-2]
  statements
endfor
```

The two expressions are evaluated in turn and should either both return integers or both return object numbers; E_TYPE is raised otherwise. The statements are then executed once for each integer (or object number, as appropriate) greater than or equal to the value of expression-1 and less than or equal to the result of expression-2, in increasing order. Each time, the given variable is assigned the integer or object number in question. For example, consider the following statements:

```
evens = {};
for n in [1..5]
  evens = {@evens, 2 * n};
endfor
```

The value of the variable `evens` after executing these statements is just as in the previous example: the list

`{2, 4, 6, 8, 10}`

The following loop over object numbers prints out the number and name of every valid object in the database:

```
for o in [#0..max_object()]
  if (valid(o))
    notify(player, tostr(o, ": ", o.name));
  endif
endfor
```

The final kind of loop in MOO executes a set of statements repeatedly as long as a given condition remains true:

```
while (expression)
  statements
endwhile
```

The expression is evaluated and, if it returns a true value, the statements are executed; then, execution of the `while` statement begins all over again with the evaluation of the expression. That is, execution alternates between evaluating the expression and executing the statements until the expression returns a false value. The following example code has precisely the same effect as the loop just shown above:

```
evens = {};
n = 1;
while (n <= 5)
  evens = {@evens, 2 * n};
  n = n + 1;
endwhile
```

Fine point: It is also possible to give a _name_ to a `while` loop.

```
while name (expression)
  statements
endwhile
```

which has precisely the same effect as

```
while (name = expression)
  statements
endwhile
```

This naming facility is only really useful in conjunction with the `break` and `continue` statements, described in the next section.

With each kind of loop, it is possible that the statements in the body of the loop will never be executed at all. For iteration over lists, this happens when the list returned by the expression is empty. For iteration on integers, it happens when expression-1 returns a larger integer than expression-2. Finally, for the `while` loop, it happens if the expression returns a false value the very first time it is evaluated.

> Warning: With `while` loops it is especially important to make sure you do not create an infinite loop. That is, a loop that will never terminate because it's expression will never become false. Be especially careful if you suspend(), yin(), or $command_utils:suspend_if_needed() within a loop, as the task may never run out of ticks.

#### Terminating One or All Iterations of a Loop

Sometimes, it is useful to exit a loop before it finishes all of its iterations. For example, if the loop is used to search for a particular kind of element of a list, then it might make sense to stop looping as soon as the right kind of element is found, even if there are more elements yet to see.  The `break` statement is used for this purpose; it has the form

```
break;
```

or

```
break name;
```

Each `break` statement indicates a specific surrounding loop; if name is not given, the statement refers to the innermost one. If it is given, name must be the name appearing right after the `for` or `while` keyword of the desired enclosing loop. When the `break` statement is executed, the indicated loop is immediately terminated and executing continues just as if the loop had completed its iterations normally.

MOO also allows you to terminate just the current iteration of a loop, making it immediately go on to the next one, if any. The `continue` statement does this; it has precisely the same forms as the `break` statement:

```
continue;
```

or

```
continue name;
```

An example that sums up a list of integers, excluding any integer equal to four:

```
my_list = {1, 2, 3, 4, 5, 6, 7};
sum = 0;
for element in (my_list)
    if (element == 4)
        continue;
    endif
    sum = sum + element;
endfor
```

An example that breaks out of hte loop when a specific object in a list is found

```
my_list = {#13633, #98, #15840, #18657, #22664};
i = 0;
found = 0;
for obj in (my_list)
    i = i + 1;
    if (obj == #18657)
        found = 1;
        break;
    endif
endfor
if (found)
    notify(player, tostr("found #18657 at ", i, " index"));
else
    notify(player, "couldn't find #18657 in the list!");
endif
```

#### Returning a Value from a Verb

The MOO program in a verb is just a sequence of statements. Normally, when the verb is called, those statements are simply executed in order and then the integer 0 is returned as the value of the verb-call expression. Using the `return` statement, one can change this behavior. The `return` statement has one of the following two forms:

```
return;
```

or

```
return expression;
```

When it is executed, execution of the current verb is terminated immediately after evaluating the given expression, if any. The verb-call expression that started the execution of this verb then returns either the value of expression or the integer 0, if no expression was provided.

We could modify the example given above. Imagine a verb called has_object which takes an object (that we want to find) as it's first argument and a list of objects (to search) as it's second argument:

```
{seek_obj, list_of_objects} = args;
for obj in (list_of_objects)
    if (obj == seek_obj)
        return 1;
    endif
endfor
```

The verb above could be called with `obj_with_verb:has_object(#18657, {#1, #3, #4, #3000})` and it would return `false` (0) if the object was not found in the list. It would return `true` (1) if the object was found in the list.

Of course we could write this much more simply (and get the index of the object in the list at the same time):

```
{seek_obj, list_of_objects} = args;
return seek_obj in list_of_objects;
```

#### Handling Errors in Statements

A traceback is raised when there is an error in the execution of code (this differs from a compilation error you might see when programming a verb).

Examples to cause tracebacks:

```
;notify(5)

#-1:Input to EVAL (this == #-1), line 3:  Incorrect number of arguments (expected 2-4; got 1)
... called from built-in function eval()
... called from #58:eval_cmd_string, line 19
... called from #58:eval*-d, line 13
(End of traceback)
```

And another example:
```
;notify(me, 5)

#-1:Input to EVAL (this == #-1), line 3:  Type mismatch (args[1] of notify() expected object; got integer)
... called from built-in function eval()
... called from #58:eval_cmd_string, line 19
... called from #58:eval*-d, line 13
(End of traceback)
```

As you can see in the above examples, ToastStunt will tell you the line number of the error, as well as some additional information about the error, including the expected number of arguments and the type. This will also work when you are catching errors in a try/except statement (detailed below).

Additional, you will also be shown {object, verb / property name} when you try to access a verb or property that was not found.

Normally, whenever a piece of MOO code raises an error, the entire task is aborted and a message printed to the user. Often, such errors can be anticipated in advance by the programmer and code written to deal with them in a more graceful manner. The `try`-`except` statement allows you to do this; the syntax is as follows:

```
try
  statements-0
except variable-1 (codes-1)
  statements-1
except variable-2 (codes-2)
  statements-2
...
endtry
```

where the variables may be omitted and each codes part is either the keyword `ANY` or else a comma-separated list of expressions, just like an argument list. As in an argument list, the splicing operator (`@`) can be used here. There can be anywhere from 1 to 255 `except` clauses.

First, each codes part is evaluated, yielding a list of error codes that should be caught if they're raised; if a codes is `ANY`, then it is equivalent to the list of all possible MOO values.

Next, statements-0 is executed; if it doesn't raise an error, then that's all that happens for the entire `try`-`except` statement. Otherwise, let E be the error it raises. From top to bottom, E is searched for in the lists resulting from the various codes parts; if it isn't found in any of them, then it continues to be raised, possibly to be caught by some piece of code either surrounding this `try`-`except` statement or higher up on the verb-call stack.

If E is found first in codes-i, then variable-i (if provided) is assigned a value containing information about the error being raised and statements-i is executed. The value assigned to variable-i is a list of four elements:

```
{code, message, value, traceback}
```

where code is E, the error being raised, message and value are as provided by the code that raised the error, and traceback is a list like that returned by the `callers()` function, including line numbers. The traceback list contains entries for every verb from the one that raised the error through the one containing this `try`-`except` statement.

Unless otherwise mentioned, all of the built-in errors raised by expressions, statements, and functions provide `tostr(code)` as message and zero as value.

Here's an example of the use of this kind of statement:

```
try
  result = object:(command)(@arguments);
  player:tell("=> ", toliteral(result));
except v (ANY)
  tb = v[4];
  if (length(tb) == 1)
    player:tell("** Illegal command: ", v[2]);
  else
    top = tb[1];
    tb[1..1] = {};
    player:tell(top[1], ":", top[2], ", line ", top[6], ":", v[2]);
    for fr in (tb)
      player:tell("... called from ", fr[1], ":", fr[2], ", line ", fr[6]);
    endfor
    player:tell("(End of traceback)");
  endif
endtry
```

#### Cleaning Up After Errors

Whenever an error is raised, it is usually the case that at least some MOO code gets skipped over and never executed. Sometimes, it's important that a piece of code _always_ be executed, whether or not an error is raised. Use the `try`-`finally` statement for these cases; it has the following syntax:

```
try
  statements-1
finally
  statements-2
endtry
```

First, statements-1 is executed; if it completes without raising an error, returning from this verb, or terminating the current iteration of a surrounding loop (we call these possibilities _transferring control_), then statements-2 is executed and that's all that happens for the entire `try`-`finally` statement.

Otherwise, the process of transferring control is interrupted and statements-2 is executed. If statements-2 itself completes without transferring control, then the interrupted control transfer is resumed just where it left off. If statements-2 does transfer control, then the interrupted transfer is simply forgotten in favor of the new one.

In short, this statement ensures that statements-2 is executed after control leaves statements-1 for whatever reason; it can thus be used to make sure that some piece of cleanup code is run even if statements-1 doesn't simply run normally to completion.

Here's an example:

```
try
  start = time();
  object:(command)(@arguments);
finally
  end = time();
  this:charge_user_for_seconds(player, end - start);
endtry
```
> Warning: If a task runs out of ticks, it's possible for your finally code to not run.

#### Executing Statements at a Later Time

It is sometimes useful to have some sequence of statements execute at a later time, without human intervention. For example, one might implement an object that, when thrown into the air, eventually falls back to the ground; the `throw` verb on that object should arrange to print a message about the object landing on the ground, but the message shouldn't be printed until some number of seconds have passed.

The `fork` statement is intended for just such situations and has the following syntax:

```
fork (expression)
  statements
endfork
```

The `fork` statement first executes the expression, which must return an integer or float; call that value n. It then creates a new MOO _task_ that will, after at least n seconds (or sub seconds in the case of a float like 0.1), execute the statements. When the new task begins, all variables will have the values they had at the time the `fork` statement was executed. The task executing the `fork` statement immediately continues execution. The concept of tasks is discussed in detail in the next section.

By default, there is no limit to the number of tasks any player may fork, but such a limit can be imposed from within the database. See the chapter on server assumptions about the database for details.

Occasionally, one would like to be able to kill a forked task before it even starts; for example, some player might have caught the object that was thrown into the air, so no message should be printed about it hitting the ground. If a variable name is given after the `fork` keyword, like this:

```
fork name (expression)
  statements
endfork
```

then that variable is assigned the _task ID_ of the newly-created task.  The value of this variable is visible both to the task executing the fork statement and to the statements in the newly-created task. This ID can be passed to the `kill_task()` function to keep the task from running and will be the value of `task_id()` once the task begins execution.

> Note: This feature has other uses as well. The MOO is single threaded (though ToastStunt supports some built-ins executing on other threads), which means that complex logic (verbs that call verbs that call verbs ...) can cause the MOO to _lag_. For instance, let's say when your user tosses their ball up, you want to calculate a complex trajectory involve the ball and other objects in the room. These calculations are costly and done in another verb, they take time to be performed. However, you want some actions to happen both before the calculations (everyone in the room seeing the ball is thrown into the air) and after the ball has left the players hands (the player reaches into their pocket and pulls out a new ball). If there is no `fork()` then the calculations need to complete before the verb can continue execution, which means the player won't pull out a fresh ball until after the calculations are complete. A `fork()` allows the player to throw the ball, the MOO to `fork()` the task, which allows execution of the verb to continue right away and the user to pull out a new ball, without experiencing the delay that the calculations being returned (without a `fork()`) would have incurred.

An example of this:

```
{ball} = args;
player:tell("You throw the ball!");
ball:calculate_trajectory();
player:tell("You get out another ball!");
```

In the above example, `player:tell("You get out another ball!");` will not be executed until after `ball:calculate_trajectory();` is completed.

```
{ball} = args;
player:tell("You throw the ball!");
fork (1)
    ball:calculate_trajectory();
endfor
player:tell("You get out another ball!");
```

In this forked example, the ball will be thrown, the task forked for 1 second later and the the final line telling the player they got out another ball will be followed up right after, without having to wait for the trajectory verb to finish running.

This type of fork cannot be used if the trajectory is required by the code that runs after it. For instance:

```
{ball} = args;
player:tell("You throw the ball!");
direction = ball:calculate_trajectory();
player:tell("You get out another ball!");
player:tell("Your ball arcs to the " + direction);
```

If the above task was forked as it is below:

```
{ball} = args;
player:tell("You throw the ball!");
fork (1)
    direction = ball:calculate_trajectory();
endfork
player:tell("You get out another ball!");
player:tell("Your ball arcs to the " + direction);
```

The verb would raise `E_VARNF` due to direction not being defined.

### MOO Tasks

A _task_ is an execution of a MOO program. There are five kinds of tasks in ToastStunt:

* Every time a player types a command, a task is created to execute that command; we call these _command tasks_.
* Whenever a player connects or disconnects from the MOO, the server starts a task to do whatever processing is necessary, such as printing out `Munchkin has connected` to all of the players in the same room; these are called _server tasks_.
* The `fork` statement in the programming language creates a task whose execution is delayed for at least some given number of seconds; these are _forked tasks_. Sub-second forking is possible (eg. 0.1)
* The `suspend()` function suspends the execution of the current task. A snapshot is taken of whole state of the execution, and the execution will be resumed later. These are called _suspended tasks_. Sub-second suspending is possible.
* The `read()` function also suspends the execution of the current task, in this case waiting for the player to type a line of input. When the line is received, the task resumes with the `read()` function returning the input line as result. These are called _reading tasks_.

The last three kinds of tasks above are collectively known as _queued tasks_ or _background tasks_, since they may not run immediately.

To prevent a maliciously- or incorrectly-written MOO program from running forever and monopolizing the server, limits are placed on the running time of every task. One limit is that no task is allowed to run longer than a certain number of seconds; command and server tasks get five seconds each while other tasks get only three seconds. This limit is, in practice, rarely reached. The reason is that there is also a limit on the number of operations a task may execute.

The server counts down _ticks_ as any task executes. Roughly speaking, it counts one tick for every expression evaluation (other than variables and literals), one for every `if`, `fork` or `return` statement, and one for every iteration of a loop. If the count gets all the way down to zero, the task is immediately and unceremoniously aborted. By default, command and server tasks begin with a store of 60,000 ticks; this is enough for almost all normal uses. Forked, suspended, and reading tasks are allotted 30,000 ticks each.

These limits on seconds and ticks may be changed from within the database, as can the behavior of the server after it aborts a task for running out; see the chapter on server assumptions about the database for details.

Because queued tasks may exist for long periods of time before they begin execution, there are functions to list the ones that you own and to kill them before they execute. These functions, among others, are discussed in the following section.

Some server functions, when given large or complicated amounts of data, may take a significant amount of time to complete their work. When this happens, the MOO can't process any other player input or background tasks and users will experience lag. To help diagnose the causes of lag, ToastStunt provides the `DEFAULT_LAG_THRESHOLD` option in options.h (which can be overridden in the database. See the Server Assumptions About the Database section). When a running task exceeds this number of seconds, the server will make a note in the server log and call the verb `#0:handle_lagging_task()` with the arguments: `{callers, execution time}`. Callers will be a `callers()`-style list of every verb call leading up to the lagging verb, and execution time will be the total time it took the verb to finish executing. This can help you gauge exactly what verb is causing the problem.

> Note: Depending on your system configuration, FG_SECONDS and BG_SECONDS may not necessarily correspond to actual seconds in real time. They often measure CPU time. This is why your verbs can lag for several seconds in real life and still not raise an 'out of seconds' error."

### Working with Anonymous Objects

Anonymous objects are typically transient and are garbage collected when they are no longer needed (IE: when nothing is referencing them).

A reference to an anonymous object is returned when the anonymous object is created with create(). References can be shared but they cannot be forged. That is, there is no literal representation of a reference to an anonymous object (that`s why they are anonymous).

Anonymous objects are created using the `create` builtin, passing the optional third argument as `1`. For example:

```
anonymous = create($thing, #2, 1);
```

Since there is no literal representation of an anonymous object, if you were to try to print it:

```
player:tell(toliteral(anonymous));
```

You would be shown: `\*anonymous\*`

You can store the reference to the anonymous object in a variable, like we did above, or you can store it in a property.

```
player.test = create($thing, player, 1)
player:tell(player.test);
```

This will also output: `\*anonymous\*`

If you store your anonymous object in a property, that anonymous object will continue to exist so long as it exists in the property. If the object with the property were recycled, or the property removed or overwritten with a different value, the anonymous object would be garbage collected.

Anonymous objects can be stored in lists:

```
my_list = {create($thing, player, 1)};
player.test = my_list;
```

The above code would result in:

```
{\*anonymous\*}
```

Anonymous objects can be stored in maps as either the key or the value:

```
[1 -> create($thing, player, 1)] => [1 -> \*anonymous\*]
[create($thing, player, 1) -> 1] => [\*anonymous\* -> 1]
```

> Warning: \*anonymous\* is not the actual key, there is not literal representation of an anonymous object reference. This means that while the object will continue to exist while it is a key of a map, the only way to reference that key would be by the reference, which you would need to store in a variable or a property. This is NOT a recommended practice, as you would have to keep a reference to the key elsewhere in order to access it (outside of iterating over all the keys).

Anonymous objects technically have a player flag and children lists, but you can't actually do anything with them. Same with the majority of the properties. They exist but are meaningless. Generally speaking, this makes WAIFs a better choice in most situations, as they are lighter weight.

> Warning: Similar to WAIFs, you want to take care in how you are creating Anonymous Objects, as once they are created, if you continue to reference them in a property, you may have trouble finding them in the future, as there is no way to pull up a list of all Anonymous Objects. 

> Note: The section for [Additional Details on WAIFs](#additional-details-on-waifs) has example verbs that can be used to detect Anonymous Objects referenced in your system.

### Working with WAIFs

The MOO object structure is unique in that all classes are instances and all instances are (potentially) classes. This means that instances carry a lot of baggage that is only useful in the event that they become classes. Also, every object comes with a set of builtin properties and attributes which are primarily useful for building VR things. My idea of a lightweight object is something which is exclusively an instance. It lacks many of the things that "real MOO objects" have for their roles as classes and VR objects:

- names
- location/contents information
- children
- flags
- verb definitions
- property definitions
- explicit destruction 

Stripped to its core, then, a WAIF has the following attributes:

- class (like a parent)
- owner (for permissions information)
- property values 

A WAIF's properties and behavior are a hybrid of several existing MOO types. It is instructive to compare them:

- WAIFs are refcounted values, like LISTs. After they are created, they exist as long as they are stored in a variable or property somewhere. When the last reference is gone the WAIF is destroyed with no notice.
- There is no syntax for creating a literal WAIF. They can only be created with a builtin.
- There is no syntax for referring to an existing WAIF. You can only use one by accessing a property or a variable where it has been stored.
- WAIFs can change, like objects. When you change a WAIF, all references to the WAIF will see the change (like OBJ, unlike LIST).
- You can call verbs and reference properties on WAIFs. These are inherited from its class object (with the mapping described below).
- WAIFs are cheap to create, about the same as LISTs.
- WAIFs are small. A WAIF with all clear properties (like right after it is created) is only a few bytes longer than a LIST big enough to hold {class, owner}. If you assign a value to a property it grows the same amount a LIST would if you appended a value to it.
- WAIF property accesses are controlled like OBJ property accesses. Having a reference to a WAIF doesn't mean you can see what's inside it.
- WAIFs can never define new verbs or properties.
- WAIFs can never have any children.
- WAIFs can't change class or ownership.
- The only builtin properties of a WAIF are .owner and .class.
- WAIFs do not participate in the .location/.contents hierarchy, as manipulated by move(). A WAIF class could define these properties, however (as described below).
- WAIFs do not have OBJ flags such as .r or .wizard.
- WAIFs can be stored in MAPs
- WAIFs can't recursively reference one another but one waif can reference another waif if the other waif doesn't reference it too.

> Note: When loading a LambdaMOO database with waifs into ToastStunt for the first time, you may get errors. This is because the WAIF type in LambdaMOO doesn't match the WAIF type in ToastStunt. To fix this error, you need to do two simple things:
> 1. Start your database in LambdaMOO as you always have and evaluate this: `;typeof($waif:new())`
> 2. Start your database in ToastStunt with the `-w <result of previous eval>` command line option. For example, if `typeof($waif:new())` in LambdaMOO was 42, you would start your MOO with something like this: `./moo -w 42 my_database.db my_converted_database.db`
> After that you're done! Your database will convert all of your existing waifs and save in the new ToastStunt format. You only have to use the `-w` option one time."

#### The WAIF Verb and Property Namespace

In order to separate the verbs and properties defined for WAIFs of an object, WAIFs only inherit verbs and properties whose names begin with : (a colon). To say that another way, the following mapping is applied:

`waif:verb(@args)` becomes `waif.class:(":"+verb)(@args)`

Inside the WAIF verb (hereinafter called a _method_) the local variable `verb` does not have the additional colon. The value of `this` is the WAIF itself (it can determine what object it's on with `this.class`). If the method calls another verb on a WAIF or an OBJ, `caller` will be the WAIF.

`waif.prop` is defined by `waif.class.(":"+prop)`

The property definition provides ownership and permissions flags for the property as well as its default value, as with any OBJ. Of course the actual property value is part of the WAIF itself and can be changed during the WAIFs lifetime.

In the case of +c properties, the WAIF owner is considered to be the property owner.

In ToastCore you will find a corified reference of `$waif` which is pre-configured for you to begin creating WAIFs or Generic OBJs that you can then use to create WAIFs with. Here's @display output for the skeletal $waif:

```
Generic Waif (#118) [ ]
  Child of Root Class (#1).
  Size: 7,311 bytes at Sun Jan  2 10:37:09 2022 PST
```

This MOO OBJ `$waif` defines a verb `new` which is just like the verbs you're already familiar with. In this case, it creates a new WAIF:

```
set_task_perms(caller_perms());
w = new_waif();
w:initialize(@args);
return w;
```

Once the WAIF has been created, you can call verbs on it. Notice how the WAIF inherits `$waif::initialize`. Notice that it cannot inherit `$waif:new` because that verb's name does not start with a colon.

The generic waif is fertile (`$waif.f == 1`) so that new waif classes can be derived from it. OBJ fertility is irrelevant when creating a WAIF. The ability to do that is restricted to the object itself (since `new_waif()` always returns a WAIF of class=caller).

There is no string format for a WAIF. `tostr()` just returns {waif}. `toliteral()` currently returns some more information, but it's just for debugging purposes. There is no towaif(). If you want to refer to a WAIF you have to read it directly from a variable or a property somewhere. If you cannot read it out of a property (or call a verb that returns it) you can't access it. There is no way to construct a WAIF reference from another type.

**Map Style WAIF access**

;me.waif["cheese"]
That will call the :_index verb on the waif class with {"cheese"} as the arguments.

;me.waif["cheese"] = 17
This will call the :_set_index verb on the waif class with {"cheese", 17} as arguments.

Originally this made it easy to implement maps into LambdaMOO, since you could just have your "map waif" store a list of keys and values and have the index verbs set and get data appropriately. Then you can use them just like the native map datatype that ToastCore has now.

There are other uses, though, that make it still useful today. For example, a file abstraction WAIF. One of the things you might do is:

```
file = $file:open("thing.txt");
return file[5..19];
```

That uses :_index to parse '5..19' and ultimately pass it off to file_readlines() to return those lines. Very convenient.

#### Additional Details on WAIFs

* When a WAIF is destroyed the MOO will call the `recycle` verb on the WAIF, if it exists.
* A WAIF has its own type so you can do: `typeof(some_waif) == WAIF)``
* The waif_stats() built-in will show how many instances of each class of WAIF exist, how many WAIFs are pending recycling, and how many WAIFs in total exist
* You can access WAIF properties using `mywaif.:waif_property_name`

> Warning: Similar to Anonymous Objects you should take care in how you are creating WAIFs as it can be difficult to find the WAIFs that exist in your system and where they are referenced.

The following code can be used to find WAIFs and Anonymous Objects that exist in your database.

```
@verb $waif_utils:"find_waif_types find_anon_types" this none this
@program $waif_utils:find_waif_types
if (!caller_perms().wizard)
  return E_PERM;
endif
{data, ?class = 0} = args;
ret = {};
TYPE = verb == "find_anon_types" ? ANON | WAIF;
if (typeof(data) in {LIST, MAP})
  "Rather than wasting time iterating through the entire list, we can find if it contains any waifs with a relatively quicker index().";
  if (index(toliteral(data), "[[class = #") != 0)
    for x in (data)
      yin(0, 1000);
      ret = {@ret, @this:(verb)(x, class)};
    endfor
  endif
elseif (typeof(data) == TYPE)
  if (class == 0 || (class != 0 && (TYPE == WAIF && data.class == class || (TYPE == ANON && `parent(data) ! E_INVARG' == class))))
    ret = {@ret, data};
  endif
endif
return ret;
.


@verb me:"@find-waifs @find-anons" any any any
@program me:@find-waifs
"Provide a summary of all properties and running verb programs that contain instantiated waifs.";
"Usage: @find-waifs [<class>] [on <object>]";
"       @find-anons [<parent>] [on <object>]";
"  e.g. @find-waifs $some_waif on #123 => Find waifs of class $some_waif on #123 only.";
"       @find-waifs on #123            => Find all waifs on #123.";
"       @find-waifs $some_waif         => Find all waifs of class $some_waif.";
"The above examples also apply to @find-anons.";
if (!player.wizard)
  return E_PERM;
endif
total = class = tasks = 0;
exclude = {$spell};
find_anon = index(verb, "anon");
search_verb = tostr("find_", find_anon ? "anon" | "waif", "_types");
{min, max} = {#0, max_object()};
if (args)
  if ((match = $string_utils:match_string(argstr, "* on *")) != 0)
    class = player:my_match_object(match[1]);
    min = max = player:my_match_object(match[2]);
  elseif ((match = $string_utils:match_string(argstr, "on *")) != 0)
    min = max = player:my_match_object(match[1]);
  else
    class = player:my_match_object(argstr);
  endif
  if (!valid(max))
    return player:tell("That object doesn't exist.");
  endif
  if (class != 0 && (class == $failed_match || !valid(class) || (!find_anon && !isa(class, $waif))))
    return player:tell("That's not a valid ", find_anon ? "object parent." | "waif class.");
  endif
endif
" -- Constants (avoid #0 property lookups on each iteration of loops) -- ";
WAIF_UTILS = $waif_utils;
STRING_UTILS = $string_utils;
OBJECT_UTILS = $object_utils;
LIST_UTILS = $list_utils;
" -- ";
player:tell("Searching for ", find_anon ? "ANON" | "WAIF", " instances. This may take some time...");
start = ftime(1);
for x in [min..max]
  yin(0, 1000);
  if (!valid(x))
    continue;
  endif
  if (toint(x) % 100 == 0 && player:is_listening() == 0)
    "No point in carrying on if the player isn't even listening.";
    return;
  elseif (x in exclude)
    continue;
  endif
  for y in (OBJECT_UTILS:all_properties(x))
    yin(0, 1000);
    if (is_clear_property(x, y))
      continue y;
    endif
    match = WAIF_UTILS:(search_verb)(x.(y), class);
    if (match != {})
      total = total + 1;
      player:tell(STRING_UTILS:nn(x), "[bold][yellow].[normal](", y, ")");
      for z in (match)
        yin(0, 1000);
        player:tell("    ", `STRING_UTILS:nn(find_anon ? parent(z) | z.class) ! E_INVARG => "*INVALID*"');
      endfor
    endif
  endfor
endfor
"Search for running verb programs containing waifs / anons. But only do this when a specific object wasn't specified.";
if (min == #0 && max == max_object())
  for x in (queued_tasks(1))
    if (length(x) < 11 || x[11] == {})
      continue;
    endif
    match = WAIF_UTILS:(search_verb)(x[11], class);
    if (match != {})
      tasks = tasks + 1;
      player:tell(x[6], ":", x[7], " (task ID ", x[1], ")");
      for z in (match)
        yin(0, 1000);
        player:tell("    ", find_anon ? parent(z) | STRING_UTILS:nn(z.class));
      endfor
    endif
  endfor
endif
player:tell();
player:tell("Total: ", total, " ", total == 1 ? "property" | "properties", tasks > 0 ? tostr(" and ", tasks, " ", tasks == 1 ? "task" | "tasks") | "", " in ", ftime(1) - start, " seconds.");
.
```
### Built-in Functions

There are a large number of built-in functions available for use by MOO programmers. Each one is discussed in detail in this section. The presentation is broken up into subsections by grouping together functions with similar or related uses.

For most functions, the expected types of the arguments are given; if the actual arguments are not of these types, `E_TYPE` is raised. Some arguments can be of any type at all; in such cases, no type specification is given for the argument. Also, for most functions, the type of the result of the function is given. Some functions do not return a useful result; in such cases, the specification `none` is used. A few functions can potentially return any type of value at all; in such cases, the specification `value` is used.

Most functions take a certain fixed number of required arguments and, in some cases, one or two optional arguments. If a function is called with too many or too few arguments, `E_ARGS` is raised.

Functions are always called by the program for some verb; that program is running with the permissions of some player, usually the owner of the verb in question (it is not always the owner, though; wizards can use `set_task_perms()` to change the permissions _on the fly_). In the function descriptions below, we refer to the player whose permissions are being used as the _programmer_.

Many built-in functions are described below as raising `E_PERM` unless the programmer meets certain specified criteria. It is possible to restrict use of any function, however, so that only wizards can use it; see the chapter on server assumptions about the database for details.

#### Object-Oriented Programming

One of the most important facilities in an object-oriented programming language is ability for a child object to make use of a parent's implementation of some operation, even when the child provides its own definition for that operation. The `pass()` function provides this facility in MOO.

**Function: `pass`**

pass -- calls the verb with the same name as the current verb but as defined on the parent of the object that defines the current verb.

value `pass` (arg, ...)

Often, it is useful for a child object to define a verb that _augments_ the behavior of a verb on its parent object. For example, in the ToastCore database, the root object (which is an ancestor of every other object) defines a verb called `description` that simply returns the value of `this.description`; this verb is used by the implementation of the `look` command. In many cases, a programmer would like the
    description of some object to include some non-constant part; for example, a sentence about whether or not the object was 'awake' or 'sleeping'. This sentence should be added onto the end of the normal description. The programmer would like to have a means of calling the normal `description` verb and then appending the sentence onto the end of that description. The function `pass()` is for exactly such situations.

`pass` calls the verb with the same name as the current verb but as defined on the parent of the object that defines the current verb. The arguments given to `pass` are the ones given to the called verb and the returned value of the called verb is returned from the call to `pass`.  The initial value of `this` in the called verb is the same as in the calling verb.

Thus, in the example above, the child-object's `description` verb might have the following implementation:

```
return pass() + "  It is " + (this.awake ? "awake." | "sleeping.");
```

That is, it calls its parent's `description` verb and then appends to the result a sentence whose content is computed based on the value of a property on the object.

In almost all cases, you will want to call `pass()` with the same arguments as were given to the current verb. This is easy to write in MOO; just call `pass(@args)`.

#### Manipulating MOO Values

There are several functions for performing primitive operations on MOO values, and they can be cleanly split into two kinds: those that do various very general operations that apply to all types of values, and those that are specific to one particular type. There are so many operations concerned with objects that we do not list them in this section but rather give them their own section following this one.

##### General Operations Applicable to All Values

**Function: `typeof`**

typeof -- Takes any MOO value and returns an integer representing the type of value.

int `typeof` (value)

The result is the same as the initial value of one of these built-in variables: `INT`, `FLOAT`, `STR`, `LIST`, `OBJ`, or `ERR`, `BOOL`, `MAP`, `WAIF`, `ANON`.  Thus, one usually writes code like this:

```
if (typeof(x) == LIST) ...
```

and not like this:

```
if (typeof(x) == 3) ...
```

because the former is much more readable than the latter.

**Function: `tostr`**

tostr -- Converts all of the given MOO values into strings and returns the concatenation of the results.

str `tostr` (value, ...)

```
tostr(17)                  =>   "17"
tostr(1.0/3.0)             =>   "0.333333333333333"
tostr(#17)                 =>   "#17"
tostr("foo")               =>   "foo"
tostr({1, 2})              =>   "{list}"
tostr([1 -> 2]             =>   "[map]"
tostr(E_PERM)              =>   "Permission denied"
tostr("3 + 4 = ", 3 + 4)   =>   "3 + 4 = 7"
```

Warning `tostr()` does not do a good job of converting lists and maps  into strings; all lists, including the empty list, are converted into the string `"{list}"` and all maps are converted into the string `"[map]"`. The function `toliteral()`, below, is better for this purpose.

**Function: `toliteral`**

Returns a string containing a MOO literal expression that, when evaluated, would be equal to value.

str `toliteral` (value)

```
toliteral(17)         =>   "17"
toliteral(1.0/3.0)    =>   "0.333333333333333"
toliteral(#17)        =>   "#17"
toliteral("foo")      =>   "\"foo\""
toliteral({1, 2})     =>   "{1, 2}"
toliteral([1 -> 2]    =>   "[1 -> 2]"
toliteral(E_PERM)     =>   "E_PERM"
```

**Function: `toint`**

toint -- Converts the given MOO value into an integer and returns that integer.

int `toint` (value)

Floating-point numbers are rounded toward zero, truncating their fractional parts. Object numbers are converted into the equivalent integers. Strings are parsed as the decimal encoding of a real number which is then converted to an integer. Errors are converted into integers obeying the same ordering (with respect to `<=` as the errors themselves. `toint()` raises `E_TYPE` if value is a list. If value is a string but the string does not contain a syntactically-correct number, then `toint()` returns 0.

```
toint(34.7)        =>   34
toint(-34.7)       =>   -34
toint(#34)         =>   34
toint("34")        =>   34
toint("34.7")      =>   34
toint(" - 34  ")   =>   -34
toint(E_TYPE)      =>   1
```

**Function: `toobj`**

toobj -- Converts the given MOO value into an object number and returns that object number.

obj `toobj` (value)

The conversions are very similar to those for `toint()` except that for strings, the number _may_ be preceded by `#`.

```
toobj("34")       =>   #34
toobj("#34")      =>   #34
toobj("foo")      =>   #0
toobj({1, 2})     =>   E_TYPE (error)
```

**Function: `tofloat`**

tofloat -- Converts the given MOO value into a floating-point number and returns that number.

float `tofloat` (value)

Integers and object numbers are converted into the corresponding integral floating-point numbers. Strings are parsed as the decimal encoding of a real number which is then represented as closely as possible as a floating-point number. Errors are first converted to integers as in `toint()` and then converted as integers are. `tofloat()` raises `E_TYPE` if value is a list. If value is a string but the string does not contain a syntactically-correct number, then `tofloat()` returns 0.

```
tofloat(34)          =>   34.0
tofloat(#34)         =>   34.0
tofloat("34")        =>   34.0
tofloat("34.7")      =>   34.7
tofloat(E_TYPE)      =>   1.0
```

**Function: `equal`**

equal -- Returns true if value1 is completely indistinguishable from value2.

int `equal` (value, value2)

This is much the same operation as `value1 == value2` except that, unlike `==`, the `equal()` function does not treat upper- and lower-case characters in strings as equal and thus, is case-sensitive.

```
"Foo" == "foo"         =>   1
equal("Foo", "foo")    =>   0
equal("Foo", "Foo")    =>   1
```

**Function: `value_bytes`**

value_bytes -- Returns the number of bytes of the server's memory required to store the given value.

int `value_bytes` (value)

**Function: `value_hash`**

value_hash -- Returns the same string as `string_hash(toliteral(value))`.

str `value_hash` (value, [, str algo] [, binary])

See the description of `string_hash()` for details.

**Function: `value_hmac`**

value_hmac -- Returns the same string as string_hmac(toliteral(value), key)

str `value_hmac` (value, STR key [, STR algo [, binary]])

See the description of string_hmac() for details.  

**Function: `generate_json`**

generate_json -- Returns the JSON representation of the MOO value.

str generate_json (value [, str mode])

Returns the JSON representation of the MOO value.

MOO supports a richer set of values than JSON allows. The optional mode specifies how this function handles the conversion of MOO values into their JSON representation.

The common subset mode, specified by the literal mode string "common-subset", is the default conversion mode. In this mode, only the common subset of types (strings and numbers) are translated with fidelity between MOO types and JSON types. All other types are treated as alternative representations of the string type. This mode is useful for integration with non-MOO applications.

The embedded types mode, specified by the literal mode string "embedded-types", adds type information. Specifically, values other than strings and numbers, which carry implicit type information, are converted into strings with type information appended. The converted string consists of the string representation of the value (as if tostr() were applied) followed by the pipe (|) character and the type. This mode is useful for serializing/deserializing objects and collections of MOO values.
    
```
generate_json([])                                           =>  "{}"
generate_json(["foo" -> "bar"])                             =>  "{\"foo\":\"bar\"}"
generate_json(["foo" -> "bar"], "common-subset")            =>  "{\"foo\":\"bar\"}"
generate_json(["foo" -> "bar"], "embedded-types")           =>  "{\"foo\":\"bar\"}"
generate_json(["foo" -> 1.1])                               =>  "{\"foo\":1.1}"
generate_json(["foo" -> 1.1], "common-subset")              =>  "{\"foo\":1.1}"
generate_json(["foo" -> 1.1], "embedded-types")             =>  "{\"foo\":1.1}"
generate_json(["foo" -> #1])                                =>  "{\"foo\":\"#1\"}"
generate_json(["foo" -> #1], "common-subset")               =>  "{\"foo\":\"#1\"}"
generate_json(["foo" -> #1], "embedded-types")              =>  "{\"foo\":\"#1|obj\"}"
generate_json(["foo" -> E_PERM])                            =>  "{\"foo\":\"E_PERM\"}"
generate_json(["foo" -> E_PERM], "common-subset")           =>  "{\"foo\":\"E_PERM\"}"
generate_json(["foo" -> E_PERM], "embedded-types")          =>  "{\"foo\":\"E_PERM|err\"}"
```

JSON keys must be strings, so regardless of the mode, the key will be converted to a string value.

```
generate_json([1 -> 2])                                     =>  "{\"1\":2}"
generate_json([1 -> 2], "common-subset")                    =>  "{\"1\":2}"
generate_json([1 -> 2], "embedded-types")                   =>  "{\"1|int\":2}"
generate_json([#1 -> 2], "embedded-types")                  =>  "{\"#1|obj\":2}"
```

> Warning: generate_json does not support WAIF or ANON types.

**Function: `parse_json`**

parse_json -- Returns the MOO value representation of the JSON string. 

value parse_json (str json [, str mode])

If the specified string is not valid JSON, E_INVARG is raised.

The optional mode specifies how this function handles conversion of MOO values into their JSON representation. The options are the same as for generate_json().

```
parse_json("{}")                                            =>  []
parse_json("{\"foo\":\"bar\"}")                             =>  ["foo" -> "bar"]
parse_json("{\"foo\":\"bar\"}", "common-subset")            =>  ["foo" -> "bar"]
parse_json("{\"foo\":\"bar\"}", "embedded-types")           =>  ["foo" -> "bar"]
parse_json("{\"foo\":1.1}")                                 =>  ["foo" -> 1.1]
parse_json("{\"foo\":1.1}", "common-subset")                =>  ["foo" -> 1.1]
parse_json("{\"foo\":1.1}", "embedded-types")               =>  ["foo" -> 1.1]
parse_json("{\"foo\":\"#1\"}")                              =>  ["foo" -> "#1"]
parse_json("{\"foo\":\"#1\"}", "common-subset")             =>  ["foo" -> "#1"]
parse_json("{\"foo\":\"#1|obj\"}", "embedded-types")        =>  ["foo" -> #1]
parse_json("{\"foo\":\"E_PERM\"}")                          =>  ["foo" -> "E_PERM"]
parse_json("{\"foo\":\"E_PERM\"}", "common-subset")         =>  ["foo" -> "E_PERM"]
parse_json("{\"foo\":\"E_PERM|err\"}", "embedded-types")    =>  ["foo" -> E_PERM]
```

In embedded types mode, key values can be converted to MOO types by appending type information. The full set of supported types are obj, str, err, float and int.
    
```
parse_json("{\"1\":2}")                                     =>   ["1" -> 2]
parse_json("{\"1\":2}", "common-subset")                    =>   ["1" -> 2]
parse_json("{\"1|int\":2}", "embedded-types")               =>   [1 -> 2]
parse_json("{\"#1|obj\":2}", "embedded-types")              =>   [#1 -> 2]
```

> Note: JSON converts `null` to the string "null". 

> Warning: WAIF and ANON types are not supported.

##### Operations on Numbers

**Function: `random`**

random -- Return a random integer

int `random` ([int mod, [int range]])

mod must be a positive integer; otherwise, `E_INVARG` is raised.  If mod is not provided, it defaults to the largest MOO integer, which will depend on if you are running 32 or 64-bit.

if range is provided then an integer in the range of mod to range (inclusive) is returned.

```
random(10)                  => integer between 1-10
random()                    => integer between 1 and maximum integer supported
random(1, 5000)             => integer between 1 and 5000
```

**Function: `frandom`**

float `frandom` (FLOAT mod1 [, FLOAT mod2)

If only one argument is given, a floating point number is chosen randomly from the range `[1.0..mod1]` and returned. If two arguments are given, a floating point number is randomly chosen from the range `[mod1..mod2]`.

**Function: `random_bytes`**

int `random_bytes` (int count)

Returns a binary string composed of between one and 10000 random bytes. count specifies the number of bytes and must be a positive integer; otherwise, E_INVARG is raised. 

**Function: `reseed_random`**

reseed_random -- Provide a new seed to the pseudo random number generator.

void `reseed_random`()

**Function: `min`**

min -- Return the smallest of it's arguments.

int `min` (int x, ...)

All of the arguments must be numbers of the same kind (i.e., either integer or floating-point); otherwise `E_TYPE` is raised.

**Function: `max`**

max -- Return the largest of it's arguments.

int `max` (int x, ...)

All of the arguments must be numbers of the same kind (i.e., either integer or floating-point); otherwise `E_TYPE` is raised.

**Function: `abs`**

abs -- Returns the absolute value of x.

int `abs` (int x)

If x is negative, then the result is `-x`; otherwise, the result is x. The number x can be either integer or floating-point; the result is of the same kind.

**Function: `exp`**

exp -- Returns E (Eulers number) raised to the power of x.

float exp (FLOAT x)

**Function: `floatstr`**

floatstr -- Converts x into a string with more control than provided by either `tostr()` or `toliteral()`.

str `floatstr` (float x, int precision [, scientific])

Precision is the number of digits to appear to the right of the decimal point, capped at 4 more than the maximum available precision, a total of 19 on most machines; this makes it possible to avoid rounding errors if the resulting string is subsequently read back as a floating-point value. If scientific is false or not provided, the result is a string in the form `"MMMMMMM.DDDDDD"`, preceded by a minus sign if and only if x is negative. If scientific is provided and true, the result is a string in the form `"M.DDDDDDe+EEE"`, again preceded by a minus sign if and only if x is negative.

**Function: `sqrt`**

sqrt -- Returns the square root of x.

float `sqrt` (float x)

Raises `E_INVARG` if x is negative.

**Function: `sin`**

sin -- Returns the sine of x.

float `sin` (float x)

**Function: `cos`**

cos -- Returns the cosine of x.

float `cos` (float x)

**Function: `tangent`**

tan -- Returns the tangent of x.

float `tan` (float x)

**Function: `asin`**

asin -- Returns the arc-sine (inverse sine) of x, in the range `[-pi/2..pi/2]`

float `asin` (float x)

Raises `E_INVARG` if x is outside the range `[-1.0..1.0]`.

**Function: `acos`**

acos -- Returns the arc-cosine (inverse cosine) of x, in the range `[0..pi]`

float `acos` (float x)

Raises `E_INVARG` if x is outside the range `[-1.0..1.0]`.

**Function: `atan`**

atan -- Returns the arc-tangent (inverse tangent) of y in the range `[-pi/2..pi/2]`.

float `atan` (float y [, float x])

if x is not provided, or of `y/x` in the range `[-pi..pi]` if x is provided.

**Function: `sinh`**

sinh -- Returns the hyperbolic sine of x.

float `sinh` (float x)

**Function: `cosh`**

cosh -- Returns the hyperbolic cosine of x.

float `cosh` (float x)

**Function: `tanh`**

tanh -- Returns the hyperbolic tangent of x.

float `tanh` (float x)

**Function: `exp`**

exp -- Returns e raised to the power of x.

float `exp` (float x)

**Function: `log`**

log -- Returns the natural logarithm of x.

float `log` (float x)

Raises `E_INVARG` if x is not positive.

**Function: `log10`**

log10 -- Returns the base 10 logarithm of x.

float `log10` (float x)

Raises `E_INVARG` if x is not positive.

**Function: `ceil`**

ceil -- Returns the smallest integer not less than x, as a floating-point number.

float `ceil` (float x)

**Function: `floor`**

floor -- Returns the largest integer not greater than x, as a floating-point number.

float `floor` (float x)

**Function: `trunc`**

trunc -- Returns the integer obtained by truncating x at the decimal point, as a floating-point number.

float `trunc` (float x)

For negative x, this is equivalent to `ceil()`; otherwise it is equivalent to `floor()`.

##### Operations on Strings

**Function: `length`**

length -- Returns the number of characters in string.

int `length` (str string)

It is also permissible to pass a list to `length()`; see the description in the next section.

```
length("foo")   =>   3
length("")      =>   0
```

**Function: `strsub`**

strsub -- Replaces all occurrences of what in subject with with, performing string substitution.

str `strsub` (str subject, str what, str with [, int case-matters])

The occurrences are found from left to right and all substitutions happen simultaneously. By default, occurrences of what are searched for while ignoring the upper/lower case distinction. If case-matters is provided and true, then case is treated as significant in all comparisons.

```
strsub("%n is a fink.", "%n", "Fred")   =>   "Fred is a fink."
strsub("foobar", "OB", "b")             =>   "fobar"
strsub("foobar", "OB", "b", 1)          =>   "foobar"
```

**Function: `index`**

**Function: `rindex`**

index -- Returns the index of the first character of the first occurrence of str2 in str1.

rindex -- Returns the index of the first character of the last occurrence of str2 in str1.

int `index` (str str1, str str2, [, int case-matters [, int skip])

int `rindex` (str str1, str str2, [, int case-matters [, int skip])

These functions will return zero if str2 does not occur in str1 at all.

By default the search for an occurrence of str2 is done while ignoring the upper/lower case distinction. If case-matters is provided and true, then case is treated as significant in all comparisons.

By default the search starts at the beginning (end) of str1. If skip is provided, the search skips the first (last) skip characters and starts at an offset from the beginning (end) of str1. The skip must be a positive integer for index() and a negative integer for rindex(). The default value of skip is 0 (skip no characters).

```
index("foobar", "o")            ⇒   2
index("foobar", "o", 0, 0)      ⇒   2
index("foobar", "o", 0, 2)      ⇒   1
rindex("foobar", "o")           ⇒   3
rindex("foobar", "o", 0, 0)     ⇒   3
rindex("foobar", "o", 0, -4)    ⇒   2
index("foobar", "x")            ⇒   0
index("foobar", "oba")          ⇒   3
index("Foobar", "foo", 1)       ⇒   0
```

**Function: `strtr`**

strtr -- Transforms the string source by replacing the characters specified by str1 with the corresponding characters specified by str2.

int `strtr` (str source, str str1, str str2 [, case-matters])

All other characters are not transformed. If str2 has fewer characters than str1 the unmatched characters are simply removed from source. By default the transformation is done on both upper and lower case characters no matter the case. If case-matters is provided and true, then case is treated as significant.

```
strtr("foobar", "o", "i")           ⇒    "fiibar"
strtr("foobar", "ob", "bo")         ⇒    "fbboar"
strtr("foobar", "", "")             ⇒    "foobar"
strtr("foobar", "foba", "")         ⇒    "r"
strtr("5xX", "135x", "0aBB", 0)     ⇒    "BbB"
strtr("5xX", "135x", "0aBB", 1)     ⇒    "BBX"
strtr("xXxX", "xXxX", "1234", 0)    ⇒    "4444"
strtr("xXxX", "xXxX", "1234", 1)    ⇒    "3434"
```

**Function: `strcmp`**

strcmp -- Performs a case-sensitive comparison of the two argument strings.

int `strcmp` (str str1, str str2)

If str1 is [lexicographically](https://en.wikipedia.org/wiki/Lexicographical_order) less than str2, the `strcmp()` returns a negative integer. If the two strings are identical, `strcmp()` returns zero. Otherwise, `strcmp()` returns a positive integer. The ASCII character ordering is used for the comparison.

**Function: `explode`**

explode -- Returns a list of substrings of subject that are separated by break. break defaults to a space.

list  `explode`(STR subject [, STR break [, INT include-sequential-occurrences])

Only the first character of `break` is considered:

```
explode("slither%is%wiz", "%")      => {"slither", "is", "wiz"}
explode("slither%is%%wiz", "%%")    => {"slither", "is", "wiz"}
```

You can use include-sequential-occurrences to get back an empty string as part of your list if `break` appears multiple times with nothing between it, or there is a leading/trailing `break` in your string:

```
explode("slither%is%%wiz", "%%", 1)  => {"slither", "is", "", "wiz"}
explode("slither%is%%wiz%", "%", 1)  => {"slither", "is", "", "wiz", ""}
explode("%slither%is%%wiz%", "%", 1) => {"", "slither", "is", "", "wiz", ""}
```

> Note: This can be used as a replacement for `$string_utils:explode`.

**Function: `decode_binary`**

decode_binary -- Returns a list of strings and/or integers representing the bytes in the binary string bin_string in order.

list `decode_binary` (str bin-string [, int fully])

If fully is false or omitted, the list contains an integer only for each non-printing, non-space byte; all other characters are grouped into the longest possible contiguous substrings. If  fully is provided and true, the list contains only integers, one for each byte represented in bin_string. Raises `E_INVARG` if bin_string is not a properly-formed binary string. (See the early section on MOO value types for a full description of binary strings.)

```
decode_binary("foo")               =>   {"foo"}
decode_binary("~~foo")             =>   {"~foo"}
decode_binary("foo~0D~0A")         =>   {"foo", 13, 10}
decode_binary("foo~0Abar~0Abaz")   =>   {"foo", 10, "bar", 10, "baz"}
decode_binary("foo~0D~0A", 1)      =>   {102, 111, 111, 13, 10}
```

**Function: `encode_binary`**

encode_binary -- Translates each integer and string in turn into its binary string equivalent, returning the concatenation of all these substrings into a single binary string.

str `encode_binary` (arg, ...)

Each argument must be an integer between 0 and 255, a string, or a list containing only legal arguments for this function. This function   (See the early section on MOO value types for a full description of binary strings.)

```
encode_binary("~foo")                     =>   "~7Efoo"
encode_binary({"foo", 10}, {"bar", 13})   =>   "foo~0Abar~0D"
encode_binary("foo", 10, "bar", 13)       =>   "foo~0Abar~0D"
```

**Function: `decode_base64`**

decode_base64 -- Returns the binary string representation of the supplied Base64 encoded string argument.

str `decode_base64` (str base64 [, int safe])

Raises E_INVARG if base64 is not a properly-formed Base64 string. If safe is provide and is true, a URL-safe version of Base64 is used (see RFC4648).

```
decode_base64("AAEC")      ⇒    "~00~01~02"
decode_base64("AAE", 1)    ⇒    "~00~01"
```

**Function: `encode_base64`**

encode_base64 -- Returns the Base64 encoded string representation of the supplied binary string argument.

str `encode_base64` (str binary [, int safe])

Raises E_INVARG if binary is not a properly-formed binary string. If safe is provide and is true, a URL-safe version of Base64 is used (see [RFC4648](https://datatracker.ietf.org/doc/html/rfc4648)).

```
encode_base64("~00~01~02")    ⇒    "AAEC"
encode_base64("~00~01", 1)    ⇒    "AAE"
```

**Function: `spellcheck`**

spellcheck -- This function checks the English spelling of word.

int | list `spellcheck`(STR word)

If the spelling is correct, the function will return a 1. If the spelling is incorrect, a LIST of suggestions for correct spellings will be returned instead. If the spelling is incorrect and no suggestions can be found, an empty LIST is returned.

**Function: `chr`**

chr -- This function translates integers into ASCII characters. Each argument must be an integer between 0 and 255.

int `chr`(INT arg, ...)

If the programmer is not a wizard, and integers less than 32 are provided, E_INVARG is raised. This prevents control characters or newlines from being written to the database file by non-trusted individuals.

**Function: `match`**

match --  Searches for the first occurrence of the regular expression pattern in the string subject

list `match` (str subject, str pattern [, int case-matters])

If pattern is syntactically malformed, then `E_INVARG` is raised.  The process of matching can in some cases consume a great deal of memory in the server; should this memory consumption become excessive, then the matching process is aborted and `E_QUOTA` is raised.

If no match is found, the empty list is returned; otherwise, these functions return a list containing information about the match (see below). By default, the search ignores upper-/lower-case distinctions. If case-matters is provided and true, then case is treated as significant in all comparisons.

The list that `match()` returns contains the details about the match made. The list is in the form:

```
{start, end, replacements, subject}
```

where start is the index in subject of the beginning of the match, end is the index of the end of the match, replacements is a list described below, and subject is the same string that was given as the first argument to `match()`.

The replacements list is always nine items long, each item itself being a list of two integers, the start and end indices in string matched by some parenthesized sub-pattern of pattern. The first item in replacements carries the indices for the first parenthesized sub-pattern, the second item carries those for the second sub-pattern, and so on. If there are fewer than nine parenthesized sub-patterns in pattern, or if some sub-pattern was not used in the match, then the corresponding item in replacements is the list {0, -1}. See the discussion of `%)`, below, for more information on parenthesized sub-patterns.

```
match("foo", "^f*o$")        =>  {}
match("foo", "^fo*$")        =>  {1, 3, {{0, -1}, ...}, "foo"}
match("foobar", "o*b")       =>  {2, 4, {{0, -1}, ...}, "foobar"}
match("foobar", "f%(o*%)b")
        =>  {1, 4, {{2, 3}, {0, -1}, ...}, "foobar"}
```

**Function: `rmatch`**

rmatch --  Searches for the last occurrence of the regular expression pattern in the string subject

list `rmatch` (str subject, str pattern [, int case-matters])

If pattern is syntactically malformed, then `E_INVARG` is raised.  The process of matching can in some cases consume a great deal of memory in the server; should this memory consumption become excessive, then the matching process is aborted and `E_QUOTA` is raised.

If no match is found, the empty list is returned; otherwise, these functions return a list containing information about the match (see below). By default, the search ignores upper-/lower-case distinctions. If case-matters is provided and true, then case is treated as significant in all comparisons.

The list that `match()` returns contains the details about the match made. The list is in the form:

```
{start, end, replacements, subject}
```

where start is the index in subject of the beginning of the match, end is the index of the end of the match, replacements is a list described below, and subject is the same string that was given as the first argument to `match()`.

The replacements list is always nine items long, each item itself being a list of two integers, the start and end indices in string matched by some parenthesized sub-pattern of pattern. The first item in replacements carries the indices for the first parenthesized sub-pattern, the second item carries those for the second sub-pattern, and so on. If there are fewer than nine parenthesized sub-patterns in pattern, or if some sub-pattern was not used in the match, then the corresponding item in replacements is the list {0, -1}. See the discussion of `%)`, below, for more information on parenthesized sub-patterns.

```
rmatch("foobar", "o*b")      =>  {4, 4, {{0, -1}, ...}, "foobar"}
```

##### Perl Compatible Regular Expressions

ToastStunt has two methods of operating on regular expressions. The classic style (outdated, more difficult to use, detailed in the next section) and the preferred Perl Compatible Regular Expression library. It is beyond the scope of this document to teach regular expressions, but an internet search should provide all the information you need to get started on what will surely become a lifelong journey of either love or frustration.

ToastCore offers two primary methods of interacting with regular expressions.

**Function: `pcre_match`**

pcre_match -- The function `pcre_match()` searches `subject` for `pattern` using the Perl Compatible Regular Expressions library. 

LIST `pcre_match`(STR subject, STR pattern [, ?case matters=0] [, ?repeat until no matches=1])

The return value is a list of maps containing each match. Each returned map will have a key which corresponds to either a named capture group or the number of the capture group being matched. The full match is always found in the key "0". The value of each key will be another map containing the keys 'match' and 'position'. Match corresponds to the text that was matched and position will return the indices of the substring within `subject`.

If `repeat until no matches` is 1, the expression will continue to be evaluated until no further matches can be found or it exhausts the iteration limit. This defaults to 1.

Additionally, wizards can control how many iterations of the loop are possible by adding a property to $server_options. $server_options.pcre_match_max_iterations is the maximum number of loops allowed before giving up and allowing other tasks to proceed. CAUTION: It's recommended to keep this value fairly low. The default value is 1000. The minimum value is 100.

Examples:

Extract dates from a string:

```
pcre_match("09/12/1999 other random text 01/21/1952", "([0-9]{2})/([0-9]{2})/([0-9]{4})")

=> {["0" -> ["match" -> "09/12/1999", "position" -> {1, 10}], "1" -> ["match" -> "09", "position" -> {1, 2}], "2" -> ["match" -> "12", "position" -> {4, 5}], "3" -> ["match" -> "1999", "position" -> {7, 10}]], ["0" -> ["match" -> "01/21/1952", "position" -> {30, 39}], "1" -> ["match" -> "01", "position" -> {30, 31}], "2" -> ["match" -> "21", "position" -> {33, 34}], "3" -> ["match" -> "1952", "position" -> {36, 39}]]}
```

Explode a string (albeit a contrived example):

```
;;ret = {}; for x in (pcre_match("This is a string of words, with punctuation, that should be exploded. By space. --zippy--", "[a-zA-Z]+", 0, 1)) ret = {@ret, x["0"]["match"]}; endfor return ret;

=> {"This", "is", "a", "string", "of", "words", "with", "punctuation", "that", "should", "be", "exploded", "By", "space", "zippy"}
```

**Function: `pcre_replace`**

pcre_replace -- The function `pcre_replace()` replaces `subject` with replacements found in `pattern` using the Perl Compatible Regular Expressions library.

STR `pcre_replace` (STR `subject`, STR `pattern`)

The pattern string has a specific format that must be followed, which should be familiar if you have used the likes of Vim, Perl, or sed. The string is composed of four elements, each separated by a delimiter (typically a slash (/) or an exclamation mark (!)), that tell PCRE how to parse your replacement. We'll break the string down and mention relevant options below:

1. Type of search to perform. In MOO, only 's' is valid. This parameter is kept for the sake of consistency.

2. The text you want to search for a replacement.

3. The regular expression you want to use for your replacement text.

4. Optional modifiers:
    * Global. This will replace all occurrences in your string rather than stopping at the first.
    * Case-insensitive. Uppercase, lowercase, it doesn't matter. All will be replaced.

Examples:

Replace one word with another:

```
pcre_replace("I like banana pie. Do you like banana pie?", "s/banana/apple/g")

=> "I like apple pie. Do you like apple pie?"
```

If you find yourself wanting to replace a string that contains slashes, it can be useful to change your delimiter to an exclamation mark:

```
pcre_replace("Unix, wow! /bin/bash is a thing.", "s!/bin/bash!/bin/fish!g")

=> "Unix, wow! /bin/fish is a thing."
```

##### Legacy MOO Regular Expressions

_Regular expression_ matching allows you to test whether a string fits into a specific syntactic shape. You can also search a string for a substring that fits a pattern.

A regular expression describes a set of strings. The simplest case is one that describes a particular string; for example, the string `foo` when regarded as a regular expression matches `foo` and nothing else. Nontrivial regular expressions use certain special constructs so that they can match more than one string. For example, the regular expression `foo%|bar` matches either the string `foo` or the string `bar`; the regular expression `c[ad]*r` matches any of the strings `cr`, `car`, `cdr`, `caar`, `cadddar` and all other such strings with any number of `a`'s and `d`'s.

Regular expressions have a syntax in which a few characters are special constructs and the rest are _ordinary_. An ordinary character is a simple regular expression that matches that character and nothing else. The special characters are `$`, `^`, `.`, `*`, `+`, `?`, `[`, `]` and `%`. Any other character appearing in a regular expression is ordinary, unless a `%` precedes it.

For example, `f` is not a special character, so it is ordinary, and therefore `f` is a regular expression that matches the string `f` and no other string. (It does _not_, for example, match the string `ff`.)  Likewise, `o` is a regular expression that matches only `o`.

Any two regular expressions a and b can be concatenated. The result is a regular expression which matches a string if a matches some amount of the beginning of that string and b matches the rest of the string.

As a simple example, we can concatenate the regular expressions `f` and `o` to get the regular expression `fo`, which matches only the string `fo`. Still trivial.

The following are the characters and character sequences that have special meaning within regular expressions. Any character not mentioned here is not special; it stands for exactly itself for the purposes of searching and matching.

| Character Sequences   | Special Meaning                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |                                                                                                                 |                                                                                     |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| <code>.</code>        | is a special character that matches any single character. Using concatenation, we can make regular expressions like <code>a.b</code>, which matches any three-character string that begins with <code>a</code> and ends with <code>b</code>.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |                                                                                                                 |                                                                                     |
| <code>*</code>        | is not a construct by itself; it is a suffix that means that the preceding regular expression is to be repeated as many times as possible. In <code>fo*</code>, the <code>*</code> applies to the <code>o</code>, so <code>fo*</code> matches <code>f</code> followed by any number of <code>o</code>&apos;s. The case of zero <code>o</code>&apos;s is allowed: <code>fo*</code> does match <code>f</code>.  <code>*</code> always applies to the <em>smallest</em> possible preceding expression.  Thus, <code>fo*</code> has a repeating <code>o</code>, not a repeating <code>fo</code>.  The matcher processes a <code>*</code> construct by matching, immediately, as many repetitions as can be found. Then it continues with the rest of the pattern.  If that fails, it backtracks, discarding some of the matches of the <code>*</code>&apos;d construct in case that makes it possible to match the rest of the pattern. For example, matching <code>c[ad]*ar</code> against the string <code>caddaar</code>, the <code>[ad]*</code> first matches <code>addaa</code>, but this does not allow the next <code>a</code> in the pattern to match. So the last of the matches of <code>[ad]</code> is undone and the following <code>a</code> is tried again. Now it succeeds.                                                                                                     |                                                                                                                 |                                                                                     |
| <code>+</code>        | <code>+</code> is like <code>*</code> except that at least one match for the preceding pattern is required for <code>+</code>. Thus, <code>c[ad]+r</code> does not match <code>cr</code> but does match anything else that <code>c[ad]*r</code> would match.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |                                                                                                                 |                                                                                     |
| <code>?</code>        | <code>?</code> is like <code>*</code> except that it allows either zero or one match for the preceding pattern. Thus, <code>c[ad]?r</code> matches <code>cr</code> or <code>car</code> or <code>cdr</code>, and nothing else.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |                                                                                                                 |                                                                                     |
| <code>[ ... ]</code>  | <code>[</code> begins a <em>character set</em>, which is terminated by a <code>]</code>. In the simplest case, the characters between the two brackets form the set. Thus, <code>[ad]</code> matches either <code>a</code> or <code>d</code>, and <code>[ad]*</code> matches any string of <code>a</code>&apos;s and <code>d</code>&apos;s (including the empty string), from which it follows that <code>c[ad]*r</code> matches <code>car</code>, etc.<br>Character ranges can also be included in a character set, by writing two characters with a <code>-</code> between them. Thus, <code>[a-z]</code> matches any lower-case letter. Ranges may be intermixed freely with individual characters, as in <code>[a-z$%.]</code>, which matches any lower case letter or <code>$</code>, <code>%</code> or period.<br> Note that the usual special characters are not special any more inside a character set. A completely different set of special characters exists inside character sets: <code>]</code>, <code>-</code> and <code>^</code>.<br> To include a <code>]</code> in a character set, you must make it the first character.  For example, <code>[]a]</code> matches <code>]</code> or <code>a</code>. To include a <code>-</code>, you must use it in a context where it cannot possibly indicate a range: that is, as the first character, or immediately after a range. |                                                                                                                 |                                                                                     |
| <code>[^ ... ]</code> | <code>[^</code> begins a <em>complement character set</em>, which matches any character except the ones specified. Thus, <code>[^a-z0-9A-Z]</code> matches all characters <em>except</em> letters and digits.<br><code>^</code> is not special in a character set unless it is the first character.  The character following the <code>^</code> is treated as if it were first (it may be a <code>-</code> or a <code>]</code>).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |                                                                                                                 |                                                                                     |
| <code>^</code>        | is a special character that matches the empty string -- but only if at the beginning of the string being matched. Otherwise it fails to match anything.  Thus, <code>^foo</code> matches a <code>foo</code> which occurs at the beginning of the string.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |                                                                                                                 |                                                                                     |
| <code>$</code>        | is similar to <code>^</code> but matches only at the <em>end</em> of the string. Thus, <code>xx*$</code> matches a string of one or more <code>x</code>&apos;s at the end of the string.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |                                                                                                                 |                                                                                     |
| <code>%</code>        | has two functions: it quotes the above special characters (including <code>%</code>), and it introduces additional special constructs.<br> Because <code>%</code> quotes special characters, <code>%$</code> is a regular expression that matches only <code>$</code>, and <code>%[</code> is a regular expression that matches only <code>[</code>, and so on.<br> For the most part, <code>%</code> followed by any character matches only that character. However, there are several exceptions: characters that, when preceded by <code>%</code>, are special constructs. Such characters are always ordinary when encountered on their own.<br>  No new special characters will ever be defined. All extensions to the regular expression syntax are made by defining new two-character constructs that begin with <code>%</code>.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |                                                                                                                 |                                                                                     |
| <code>%\|</code>      | specifies an alternative. Two regular expressions a and b with <code>%                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | </code> in between form an expression that matches anything that either a or b will match.<br> Thus, <code>foo% | bar</code> matches either <code>foo</code> or <code>bar</code> but no other string. |
<code>%|</code> applies to the largest possible surrounding expressions. Only a surrounding <code>%( ... %)</code> grouping can limit the grouping power of <code>%|</code>.<br> Full backtracking capability exists for when multiple <code>%|</code>&apos;s are used. |
| <code>%( ... %)</code> | is a grouping construct that serves three purposes:<br> * To enclose a set of <code>%\|</code> alternatives for other operations. Thus, <code>%(foo%\|bar%)x</code> matches either <code>foox</code> or <code>barx</code>.<br> * To enclose a complicated expression for a following <code>*</code>, <code>+</code>, or <code>?</code> to operate on. Thus, <code>ba%(na%)*</code> matches <code>bananana</code>, etc., with any number of <code>na</code>&apos;s, including none.<br> * To mark a matched substring for future reference.<br> This last application is not a consequence of the idea of a parenthetical grouping; it is a separate feature that happens to be assigned as a second meaning to the same <code>%( ... %)</code> construct because there is no conflict in practice between the two meanings. Here is an explanation of this feature:                                                                                                                                                                               |                                                                                                                                                                                                               |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| <code>%digit</code>    | After the end of a <code>%( ... %)</code> construct, the matcher remembers the beginning and end of the text matched by that construct. Then, later on in the regular expression, you can use <code>%</code> followed by digit to mean &quot;match the same text matched by the digit&apos;th <code>%( ... %)</code> construct in the pattern.&quot;  The <code>%( ... %)</code> constructs are numbered in the order that their <code>%(</code>&apos;s appear in the pattern.<br> The strings matching the first nine <code>%( ... %)</code> constructs appearing in a regular expression are assigned numbers 1 through 9 in order of their beginnings. <code>%1</code> through <code>%9</code> may be used to refer to the text matched by the corresponding <code>%( ... %)</code> construct.<br> For example, <code>%(.*%)%1</code> matches any string that is composed of two identical halves. The <code>%(.*%)</code> matches the first half, which may be anything, but the <code>%1</code> that follows must match the same exact text. |                                                                                                                                                                                                               |
| <code>%b</code>        | matches the empty string, but only if it is at the beginning or end of a word. Thus, <code>%bfoo%b</code> matches any occurrence of <code>foo</code> as a separate word. <code>%bball%(s%                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | %)%b</code> matches <code>ball</code> or <code>balls</code> as a separate word.<br> For the purposes of this construct and the five that follow, a word is defined to be a sequence of letters and/or digits. |
| <code>%B</code>        | matches the empty string, provided it is <em>not</em> at the beginning or end of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |                                                                                                                                                                                                               |
| <code>%&lt;</code>     | matches the empty string, but only if it is at the beginning of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |                                                                                                                                                                                                               |
| <code>%&gt;</code>     | matches the empty string, but only if it is at the end of a word.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |                                                                                                                                                                                                               |
| <code>%w</code>        | matches any word-constituent character (i.e., any letter or digit).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |                                                                                                                                                                                                               |
| <code>%W</code>        | matches any character that is not a word constituent.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |                                                                                                                                                                                                               |

**Function: `substitute`**

substitute -- Performs a standard set of substitutions on the string template, using the information contained in subs, returning the resulting, transformed template.

str `substitute` (str template, list subs)

Subs should be a list like those returned by `match()` or `rmatch()` when the match succeeds; otherwise, `E_INVARG` is raised.

In template, the strings `%1` through `%9` will be replaced by the text matched by the first through ninth parenthesized sub-patterns when `match()` or `rmatch()` was called. The string `%0` in template will be replaced by the text matched by the pattern as a whole when `match()` or `rmatch()` was called. The string `%%` will be replaced by a single `%` sign. If `%` appears in template followed by any other character, `E_INVARG` will be raised.

```
subs = match("*** Welcome to ToastStunt!!!", "%(%w*%) to %(%w*%)");
substitute("I thank you for your %1 here in %2.", subs)
        =>   "I thank you for your Welcome here in ToastStunt."
```

**Function: `salt`**

salt -- Generate a crypt() compatible salt string for the specified salt format using the specified binary random input.

str `salt` (str format, str input)

The specific set of formats supported depends on the libraries used to build the server, but will always include the standard salt format, indicated by the format string "" (the empty string), and the BCrypt salt format, indicated by the format string "$2a$NN$" (where "NN" is the work factor). Other possible formats include MD5 ("$1$"), SHA256 ("$5$") and SHA512 ("$6$"). Both the SHA256 and SHA512 formats support optional rounds.

```
salt("", ".M")                                           ⇒    "iB"
salt("$1$", "~183~1E~C6/~D1")                            ⇒    "$1$MAX54zGo"
salt("$5$", "x~F2~1Fv~ADj~92Y~9E~D4l~C3")                ⇒    "$5$s7z5qpeOGaZb"
salt("$5$rounds=2000$", "G~7E~A7~F5Q5~B7~0Aa~80T")       ⇒    "$5$rounds=2000$5trdp5JBreEM"
salt("$6$", "U7~EC!~E8~85~AB~CD~B5+~E1?")                ⇒    "$6$JR1vVUSVfqQhf2yD"
salt("$6$rounds=5000$", "~ED'~B0~BD~B9~DB^,\\~BD~E7")    ⇒    "$6$rounds=5000$hT0gxavqSl0L"
salt("$2a$08$", "|~99~86~DEq~94_~F3-~1A~D2#~8C~B5sx")    ⇒    "$2a$08$dHkE1lESV9KrErGhhJTxc."
```

> Note: To ensure proper security, the random input must be from a sufficiently random source.

**Function: `crypt`**

crypt -- Encrypts the given text using the standard UNIX encryption method.

str `crypt` (str text [, str salt])

Encrypts (hashes) the given text using the standard UNIX encryption method. If provided, salt should be a string at least two characters long, and it may dictate a specific algorithm to use. By default, crypt uses the original, now insecure, DES algorithm. ToastStunt specifically includes the BCrypt algorithm (identified by salts that start with "$2a$"), and may include MD5, SHA256, and SHA512 algorithms depending on the libraries used to build the server. The salt used is returned as the first part of the resulting encrypted string.

Aside from the possibly-random input in the salt, the encryption algorithms are entirely deterministic. In particular, you can test whether or not a given string is the same as the one used to produce a given piece of encrypted text; simply extract the salt from the front of the encrypted text and pass the candidate string and the salt to crypt(). If the result is identical to the given encrypted text, then you`ve got a match.

```
crypt("foobar", "iB")                               ⇒    "iBhNpg2tYbVjw"
crypt("foobar", "$1$MAX54zGo")                      ⇒    "$1$MAX54zGo$UKU7XRUEEiKlB.qScC1SX0"
crypt("foobar", "$5$s7z5qpeOGaZb")                  ⇒    "$5$s7z5qpeOGaZb$xkxjnDdRGlPaP7Z ... .pgk/pXcdLpeVCYh0uL9"
crypt("foobar", "$5$rounds=2000$5trdp5JBreEM")      ⇒    "$5$rounds=2000$5trdp5JBreEM$Imi ... ckZPoh7APC0Mo6nPeCZ3"
crypt("foobar", "$6$JR1vVUSVfqQhf2yD")              ⇒    "$6$JR1vVUSVfqQhf2yD$/4vyLFcuPTz ... qI0w8m8az076yMTdl0h."
crypt("foobar", "$6$rounds=5000$hT0gxavqSl0L")      ⇒    "$6$rounds=5000$hT0gxavqSl0L$9/Y ... zpCATppeiBaDxqIbAN7/"
crypt("foobar", "$2a$08$dHkE1lESV9KrErGhhJTxc.")    ⇒    "$2a$08$dHkE1lESV9KrErGhhJTxc.QnrW/bHp8mmBl5vxGVUcsbjo3gcKlf6"
```

> Note: The specific set of supported algorithms depends on the libraries used to build the server. Only the BCrypt algorithm, which is distributed with the server source code, is guaranteed to exist. BCrypt is currently mature and well tested, and is recommended for new development when the Argon2 library is unavailable. (See next section).

> Warning: The entire salt (of any length) is passed to the operating system`s low-level crypt function. It is unlikely, however, that all operating systems will return the same string when presented with a longer salt. Therefore, identical calls to crypt() may generate different results on different platforms, and your password verification systems will fail. Use a salt longer than two characters at your own risk. 

**Function: `argon2`**

argon2 -- Hashes a password using the Argon2id password hashing algorithm.

The function `argon2()' hashes a password using the Argon2id password hashing algorithm. It is parametrized by three optional arguments:

str `argon2` (STR password, STR salt [, iterations = 3] [, memory usage in KB = 4096] [, CPU threads = 1])

 * Time: This is the number of times the hash will get run. This defines the amount of computation required and, as a result, how long the function will take to complete.
 * Memory: This is how much RAM is reserved for hashing.
 * Parallelism: This is the number of CPU threads that will run in parallel.

The salt for the password should, at minimum, be 16 bytes for password hashing. It is recommended to use the random_bytes() function.

```
salt = random_bytes(20);
return argon2(password, salt, 3, 4096, 1);
```

> Warning: The MOO is single threaded in most cases, and this function can take significant time depending on how you call it. While it is working, nothing else is going to be happening on your MOO. It is possible to build the server with the `THREAD_ARGON2` option which will mitigate lag. This has major caveats however, see the section below on `argon2_verify` for more information.

**Function: `argon2_verify`**

argon2_verify -- Compares password to the previously hashed hash. 

int argon2_verify (STR hash, STR password)

Returns 1 if the two match or 0 if they don't. 

This is a more secure way to hash passwords than the `crypt()` builtin.

> Note: ToastCore defines some sane defaults for how to utilize `argon2` and `argon2_verify`. You can `@grep argon2` from within ToastCore to find these.

> Warning: It is possible to build the server with the `THREAD_ARGON2` option. This will enable this built-in to run in a background thread and mitigate lag that these functions can cause. However, this comes with some major caveats. `do_login_command` (where you will typically be verifying passwords) cannot be suspended. Since threading implicitly suspends the MOO task, you won't be able to directly use Argon2 in do_login_command. Instead, you'll have to devise a new solution for logins that doesn't directly involve calling Argon2 in do_login_command.

> Note: More information on Argon2 can be found in the [Argon2 Github](https://github.com/P-H-C/phc-winner-argon2).

**Function: `string_hash`**

**Function: `binary_hash`**

string_hash -- Returns a string encoding the result of applying the SHA256 cryptographically secure hash function to the contents of the string text or the binary string bin-string.

binary_hash -- Returns a string encoding the result of applying the SHA256 cryptographically secure hash function to the contents of the string text or the binary string bin-string.

str `string_hash` (str string, [, algo [, binary]]) 

str `binary_hash` (str bin-string, [, algo [, binary])

 If algo is provided, it specifies the hashing algorithm to use. "MD5", "SHA1", "SHA224", "SHA256", "SHA384", "SHA512" and "RIPEMD160" are all supported. If binary is provided and true, the result is in MOO binary string format; by default the result is a hexadecimal string.

Note that the MD5 hash algorithm is broken from a cryptographic standpoint, as is SHA1. Both are included for interoperability with existing applications (both are still popular).

All supported hash functions have the property that, if

`string_hash(x) == string_hash(y)`

then, almost certainly,

`equal(x, y)`

This can be useful, for example, in certain networking applications: after sending a large piece of text across a connection, also send the result of applying string_hash() to the text; if the destination site also applies string_hash() to the text and gets the same result, you can be quite confident that the large text has arrived unchanged. 

**Function: `string_hmac`**

**Function: `binary_hmac`**

str `string_hmac` (str text, str key [, str algo [, binary]])

str binary_hmac (str bin-string, str key [, str algo [, binary]])

Returns a string encoding the result of applying the HMAC-SHA256 cryptographically secure HMAC function to the contents of the string text or the binary string bin-string with the specified secret key. If algo is provided, it specifies the hashing algorithm to use. Currently, only "SHA1" and "SHA256" are supported. If binary is provided and true, the result is in MOO binary string format; by default the result is a hexadecimal string.

All cryptographically secure HMACs have the property that, if

`string_hmac(x, a) == string_hmac(y, b)`

then, almost certainly,

`equal(x, y)`

and furthermore,

`equal(a, b)`

This can be useful, for example, in applications that need to verify both the integrity of the message (the text) and the authenticity of the sender (as demonstrated by the possession of the secret key).

##### Operations on Lists

**Function: `length`**

length -- Returns the number of elements in list.

int `length` (list list)

It is also permissible to pass a string to `length()`; see the description in the previous section.

```
length({1, 2, 3})   =>   3
length({})          =>   0
```

**Function: `is_member`**

is_member -- Returns true if there is an element of list that is completely indistinguishable from value.

int `is_member` (ANY value, LIST list [, INT case-sensitive])

This is much the same operation as " `value in list`" except that, unlike `in`, the `is_member()` function does not treat upper- and lower-case characters in strings as equal. This treatment of strings can be controlled with the `case-sensitive` argument; setting `case-sensitive` to false will effectively disable this behavior.

Raises E_ARGS if two values are given or if more than three arguments are given. Raises E_TYPE if the second argument is not a list. Otherwise returns the index of `value` in `list`, or 0 if it's not in there.

```
is_member(3, {3, 10, 11})                  => 1
is_member("a", {"A", "B", "C"})            => 0
is_member("XyZ", {"XYZ", "xyz", "XyZ"})    => 3
is_member("def", {"ABC", "DEF", "GHI"}, 0) => 2 
```

**Function: `all_members`**

all_members -- Returns the indices of every instance of `value` in `alist`.

LIST `all_members`(ANY `value`, LIST `alist`)

Example:

```
all_members("a", {"a", "b", "a", "c", "a", "d"}) => {1, 3, 5}
```

**Function: `listinsert`**

**Function: `listappend`**

listinsert -- This functions return a copy of list with value added as a new element.

listappend -- This functions return a copy of list with value added as a new element.

list `listinsert` (list list, value [, int index]) list `listappend` (list list, value [, int index])

`listinsert()` and `listappend()` add value before and after (respectively) the existing element with the given index, if provided.

The following three expressions always have the same value:

```
listinsert(list, element, index)
listappend(list, element, index - 1)
{@list[1..index - 1], element, @list[index..length(list)]}
```

If index is not provided, then `listappend()` adds the value at the end of the list and `listinsert()` adds it at the beginning; this usage is discouraged, however, since the same intent can be more clearly expressed using the list-construction expression, as shown in the examples below.

```
x = {1, 2, 3};
listappend(x, 4, 2)   =>   {1, 2, 4, 3}
listinsert(x, 4, 2)   =>   {1, 4, 2, 3}
listappend(x, 4)      =>   {1, 2, 3, 4}
listinsert(x, 4)      =>   {4, 1, 2, 3}
{@x, 4}               =>   {1, 2, 3, 4}
{4, @x}               =>   {4, 1, 2, 3}
```

**Function: `listdelete`**

listdelete -- Returns a copy of list with the indexth element removed.

list `listdelete` (list list, int index)

If index is not in the range `[1..length(list)]`, then `E_RANGE` is raised.

```
x = {"foo", "bar", "baz"};
listdelete(x, 2)   =>   {"foo", "baz"}
```

**Function: `listset`**

listset -- Returns a copy of list with the indexth element replaced by value.

list `listset` (list list, value, int index)

If index is not in the range `[1..length(list)]`, then `E_RANGE` is raised.

```
x = {"foo", "bar", "baz"};
listset(x, "mumble", 2)   =>   {"foo", "mumble", "baz"}
```

This function exists primarily for historical reasons; it was used heavily before the server supported indexed assignments like `x[i] = v`. New code should always use indexed assignment instead of `listset()` wherever possible.

**Function: `setadd`**<br>
**Function: `setremove`**

setadd -- Returns a copy of list with the given value added.

setremove -- Returns a copy of list with the given value removed.

list `setadd` (list list, value) list `setremove` (list list, value)

`setadd()` only adds value if it is not already an element of list; list is thus treated as a mathematical set. value is added at the end of the resulting list, if at all.  Similarly, `setremove()` returns a list identical to list if value is not an element. If value appears more than once in list, only the first occurrence is removed in the returned copy.

```
setadd({1, 2, 3}, 3)         =>   {1, 2, 3}
setadd({1, 2, 3}, 4)         =>   {1, 2, 3, 4}
setremove({1, 2, 3}, 3)      =>   {1, 2}
setremove({1, 2, 3}, 4)      =>   {1, 2, 3}
setremove({1, 2, 3, 2}, 2)   =>   {1, 3, 2}
```

**Function: `reverse`**

reverse -- Return a reversed list or string

str | list `reverse`(LIST alist)

Examples:

```
reverse({1,2,3,4}) => {4,3,2,1}
reverse("asdf") => "fdsa"
```

**Function: `slice`**

list `slice`(LIST alist [, INT | LIST | STR index, ANY default map value])

Return the index-th elements of alist. By default, index will be 1. If index is a list of integers, the returned list will have those elements from alist. This is the built-in equivalent of LambdaCore's $list_utils:slice verb.

If alist is a list of maps, index can be a string indicating a key to return from each map in alist.

If default map value is specified, any maps not containing the key index will have default map value returned in their place. This is useful in situations where you need to maintain consistency with a list index and can't have gaps in your return list.

Examples:

```
slice({{"z", 1}, {"y", 2}, {"x",5}}, 2)                                 => {1, 2, 5}
slice({{"z", 1, 3}, {"y", 2, 4}}, {2, 1})                               => {{1, "z"}, {2, "y"}}
slice({["a" -> 1, "b" -> 2], ["a" -> 5, "b" -> 6]}, "a")                => {1, 5}
slice({["a" -> 1, "b" -> 2], ["a" -> 5, "b" -> 6], ["b" -> 8]}, "a", 0) => {1, 5, 0}
```
**Function: `sort`**

sort -- Sorts list either by keys or using the list itself.

list `sort`(LIST list [, LIST keys, INT natural sort order?, INT reverse])

When sorting list by itself, you can use an empty list ({}) for keys to specify additional optional arguments.

If natural sort order is true, strings containing multi-digit numbers will consider those numbers to be a single character. So, for instance, this means that 'x2' would come before 'x11' when sorted naturally because 2 is less than 11. This argument defaults to 0.

If reverse is true, the sort order is reversed. This argument defaults to 0.

Examples:

Sort a list by itself:

```
sort({"a57", "a5", "a7", "a1", "a2", "a11"}) => {"a1", "a11", "a2", "a5", "a57", "a7"}
```

Sort a list by itself with natural sort order:

```
sort({"a57", "a5", "a7", "a1", "a2", "a11"}, {}, 1) => {"a1", "a2", "a5", "a7", "a11", "a57"}
```

Sort a list of strings by a list of numeric keys:

```
sort({"foo", "bar", "baz"}, {123, 5, 8000}) => {"bar", "foo", "baz"}
```

> Note: This is a threaded function.

##### Operations on Maps

When using the functions below, it's helpful to remember that maps are ordered.

**Function: `mapkeys`**

mapkeys -- returns the keys of the elements of a map.

list `mapkeys` (map map)

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapkeys(x)   =>  {"bar", "baz", "foo"}
```

**Function: `mapvalues`**

mapvalues -- returns the values of the elements of a map.

list `mapvalues` (MAP `map` [, ... STR `key`])

If you only want the values of specific keys in the map, you can specify them as optional arguments. See examples below.

Examples:  

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapvalues(x)               =>  {2, 3, 1}
mapvalues(x, "foo", "baz") => {1, 3}
```

**Function: `mapdelete`**
mapdelete -- Returns a copy of map with the value corresponding to key removed. If key is not a valid key, then E_RANGE is raised.

map `mapdelete` (map map, key)

```
x = ["foo" -> 1, "bar" -> 2, "baz" -> 3];
mapdelete(x, "bar")   ⇒   ["baz" -> 3, "foo" -> 1]
```

**Function: `maphaskey`**

maphaskey -- Returns 1 if key exists in map. When not dealing with hundreds of keys, this function is faster (and easier to read) than something like: !(x in mapkeys(map))

int `maphaskey` (MAP map, STR key)

#### Manipulating Objects

Objects are, of course, the main focus of most MOO programming and, largely due to that, there are a lot of built-in functions for manipulating them.

##### Fundamental Operations on Objects

**Function: `create`**

create -- Creates and returns a new object whose parent (or parents) is parent (or parents) and whose owner is as described below.

obj `create` (obj parent [, obj owner] [, int anon-flag] [, list init-args])

obj `create` (list parents [, obj owner] [, int anon-flag] [, list init-args])

Creates and returns a new object whose parents are parents (or whose parent is parent) and whose owner is as described below. If any of the given parents are not valid, or if the given parent is neither valid nor #-1, then E_INVARG is raised. The given parents objects must be valid and must be usable as a parent (i.e., their `a` or `f` bits must be true) or else the programmer must own parents or be a wizard; otherwise E_PERM is raised. Furthermore, if anon-flag is true then `a` must be true; and, if anon-flag is false or not present, then `f` must be true. Otherwise, E_PERM is raised unless the programmer owns parents or is a wizard. E_PERM is also raised if owner is provided and not the same as the programmer, unless the programmer is a wizard. 

After the new object is created, its initialize verb, if any, is called. If init-args were given, they are passed as args to initialize. The new object is assigned the least non-negative object number that has not yet been used for a created object. Note that no object number is ever reused, even if the object with that number is recycled.

> Note: This is not strictly true, especially if you are using ToastCore and the `$recycler`, which is a great idea.  If you don't, you end up with extremely high object numbers. However, if you plan on reusing object numbers you need to consider this carefully in your code. You do not want to include object numbers in your code if this is the case, as object numbers could change. Use corified references instead. For example, you can use `@corify #objnum as $my_object` and then be able to reference $my_object in your code. Alternatively you can do ` @prop $sysobj.my_object #objnum`. If the object number ever changes, you can change the reference without updating all of your code.)

> Note: $sysobj is typically #0. Though it can technically be changed to something else, there is no reason that the author knows of to break from convention here.

If anon-flag is false or not present, the new object is a permanent object and is assigned the least non-negative object number that has not yet been used for a created object. Note that no object number is ever reused, even if the object with that number is recycled.

If anon-flag is true, the new object is an anonymous object and is not assigned an object number. Anonymous objects are automatically recycled when they are no longer used.

The owner of the new object is either the programmer (if owner is not provided), the new object itself (if owner was given and is invalid, or owner (otherwise). 

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

In addition, the new object inherits all of the other properties on its parents. These properties have the same permission bits as on the parents. If the `c` permissions bit is set, then the owner of the property on the new object is the same as the owner of the new object itself; otherwise, the owner of the property on the new object is the same as that on the parent. The initial value of every inherited property is clear; see the description of the built-in function clear_property() for details.

If the intended owner of the new object has a property named `ownership_quota` and the value of that property is an integer, then create() treats that value as a quota. If the quota is less than or equal to zero, then the quota is considered to be exhausted and create() raises E_QUOTA instead of creating an object. Otherwise, the quota is decremented and stored back into the `ownership_quota` property as a part of the creation of the new object. 

> Note: In ToastStunt, this is disabled by default with the "OWNERSHIP_QUOTA" option in options.h

**Function: `owned_objects`**

owned_objects -- Returns a list of all objects in the database owned by `owner`. Ownership is defined by the value of .owner on the object.

list `owned_objects`(OBJ owner)

**Function: `chparent`**

**Function: `chparents`**

chparent -- Changes the parent of object to be new-parent.

chparents -- Changes the parent of object to be new-parents.

none `chparent` (obj object, obj new-parent)

none `chparents` (obj object, list new-parents)

If object is not valid, or if new-parent is neither valid nor equal to `#-1`, then `E_INVARG` is raised. If the programmer is neither a wizard or the owner of object, or if new-parent is not fertile (i.e., its `f` bit is not set) and the programmer is neither the owner of new-parent nor a wizard, then `E_PERM` is raised. If new-parent is equal to `object` or one of its current ancestors, `E_RECMOVE` is raised. If object or one of its descendants defines a property with the same name as one defined either on new-parent or on one of its ancestors, then `E_INVARG` is raised.

Changing an object's parent can have the effect of removing some properties from and adding some other properties to that object and all of its descendants (i.e., its children and its children's children, etc.). Let common be the nearest ancestor that object and new-parent have in common before the parent of object is changed. Then all properties defined by ancestors of object under common (that is, those ancestors of object that are in turn descendants of common) are removed from object and all of its descendants. All properties defined by new-parent or its ancestors under common are added to object and all of its descendants. As with `create()`, the newly-added properties are given the same permission bits as they have on new-parent, the owner of each added property is either the owner of the object it's added to (if the `c` permissions bit is set) or the owner of that property on new-parent, and the value of each added property is _clear_; see the description of the built-in function `clear_property()` for details. All properties that are not removed or added in the reparenting process are completely unchanged.

If new-parent is equal to `#-1`, then object is given no parent at all; it becomes a new root of the parent/child hierarchy. In this case, all formerly inherited properties on object are simply removed.

If new-parents is equal to {}, then object is given no parent at all; it becomes a new root of the parent/child hierarchy. In this case, all formerly inherited properties on object are simply removed.

> Warning: On the subject of multiple inheritance, the author (Slither) thinks you should completely avoid it. Prefer [composition over inheritance](https://en.wikipedia.org/wiki/Composition_over_inheritance).

**Function: `valid`**

valid -- Return a non-zero integer if object is valid and not yet recycled.

int `valid` (obj object)

Returns a non-zero integer (i.e., a true value) if object is a valid object (one that has been created and not yet recycled) and zero (i.e., a false value) otherwise.

```
valid(#0)    =>   1
valid(#-1)   =>   0
```

**Function: `parent`**

**Function: `parents`**

parent -- return the parent of object

parents -- return the parents of object

obj `parent` (obj object)

list `parents` (obj object)

**Function: `children`**

children -- return a list of the children of object.

list `children` (obj object)

**Function: `isa`**

int isa(OBJ object, OBJ parent)

obj isa(OBJ object, LIST parent list [, INT return_parent])

Returns true if object is a descendant of parent, otherwise false.

If a third argument is present and true, the return value will be the first parent that object1 descends from in the `parent list`.

```
isa(#2, $wiz)                           => 1
isa(#2, {$thing, $wiz, $container})     => 1
isa(#2, {$thing, $wiz, $container}, 1)  => #57 (generic wizard)
isa(#2, {$thing, $room, $container}, 1) => #-1 
```

**Function: `locate_by_name`**

locate_by_name -- This function searches every object in the database for those containing `object name` in their .name property.

list `locate_by_name` (STR object name)

> Warning: Take care when using this when thread mode is active, as this is a threaded function and that means it implicitly suspends. `set_thread_mode(0)` if you want to use this without suspending.

**Function: `locations`**

list `locations`(OBJ object [, OBJ stop [, INT is-parent]])

Recursively build a list of an object's location, its location's location, and so forth until finally hitting $nothing.

Example:

```
locations(me) => {#20381, #443, #104735}

$string_utils:title_list(locations(me)) => "\"Butterknife Ballet\" Control Room FelElk, the one-person celestial birther \"Butterknife Ballet\", and Uncharted Space: Empty Space"
```

If `stop` is in the locations found, it will stop before there and return the list (exclusive of the stop object). 

If the third argument is true, `stop` is assumed to be a PARENT. And if any of your locations are children of that parent, it stops there.

**Function: `occupants`**

list `occupants`(LIST objects [, OBJ | LIST parent, INT player flag set?])

Iterates through the list of objects and returns those matching a specific set of criteria:

1. If only objects is specified, the occupants function will return a list of objects with the player flag set.

2. If the parent argument is specified, a list of objects descending from parent> will be returned. If parent is a list, object must descend from at least one object in the list.

3. If both parent and player flag set are specified, occupants will check both that an object is descended from parent and also has the player flag set.

**Function: `recycle`**

recycle -- destroy object irrevocably.

none `recycle` (obj object)

The given object is destroyed, irrevocably. The programmer must either own object or be a wizard; otherwise, `E_PERM` is raised. If object is not valid, then `E_INVARG` is raised. The children of object are reparented to the parent of object. Before object is recycled, each object in its contents is moved to `#-1` (implying a call to object's `exitfunc` verb, if any) and then object's `recycle` verb, if any, is called with no arguments.

After object is recycled, if the owner of the former object has a property named `ownership_quota` and the value of that property is a integer, then `recycle()` treats that value as a _quota_ and increments it by one, storing the result back into the `ownership_quota` property.

**Function: `recreate`**

recreate -- Recreate invalid object old (one that has previously been recycle()ed) as parent, optionally owned by owner.

obj `recreate`(OBJ old, OBJ parent [, OBJ owner])

This has the effect of filling in holes created by recycle() that would normally require renumbering and resetting the maximum object.

The normal rules apply to parent and owner. You either have to own parent, parent must be fertile, or you have to be a wizard. Similarly, to change owner, you should be a wizard. Otherwise it's superfluous.

**Function: `next_recycled_object`**

next_recycled_object -- Return the lowest invalid object. If start is specified, no object lower than start will be considered. If there are no invalid objects, this function will return 0.

obj | int `next_recycled_object`(OBJ start)

**Function: `recycled_objects`**

recycled_objects -- Return a list of all invalid objects in the database. An invalid object is one that has been destroyed with the recycle() function.

list `recycled_objects`()

**Function: `ancestors`**

ancestors -- Return a list of all ancestors of `object` in order ascending up the inheritance hiearchy. If `full` is true, `object` will be included in the list.

list `ancestors`(OBJ object [, INT full])

**Function: `clear_ancestor_cache`**

void `clear_ancestor_cache`()

The ancestor cache contains a quick lookup of all of an object's ancestors which aids in expediant property lookups. This is an experimental feature and, as such, you may find that something has gone wrong. If that's that case, this function will completely clear the cache and it will be rebuilt as-needed.

**Function: `descendants`**

list `descendants`(OBJ object [, INT full])

Return a list of all nested children of object. If full is true, object will be included in the list.

**Function: `object_bytes`**

object_bytes -- Returns the number of bytes of the server's memory required to store the given object.

int `object_bytes` (obj object)

The space calculation includes the space used by the values of all of the objects non-clear properties and by the verbs and properties defined directly on the object.

Raises `E_INVARG` if object is not a valid object and `E_PERM` if the programmer is not a wizard.

**Function: `respond_to`**

int | list respond_to(OBJ object, STR verb)

Returns true if verb is callable on object, taking into account inheritance, wildcards (star verbs), etc. Otherwise, returns false.  If the caller is permitted to read the object (because the object's `r' flag is true, or the caller is the owner or a wizard) the true value is a list containing the object number of the object that defines the verb and the full verb name(s).  Otherwise, the numeric value `1' is returned.

**Function: `max_object`**

max_object -- Returns the largest object number ever assigned to a created object.

obj `max_object`()

//TODO update for how Toast handles recycled objects if it is different
Note that the object with this number may no longer exist; it may have been recycled.  The next object created will be assigned the object number one larger than the value of `max_object()`. The next object getting the number one larger than `max_object()` only applies if you are using built-in functions for creating objects and does not apply if you are using the `$recycler` to create objects.

##### Object Movement

**Function: `move`**

move -- Changes what's location to be where.

none `move` (obj what, obj where [, INT position)

This is a complex process because a number of permissions checks and notifications must be performed.  The actual movement takes place as described in the following paragraphs.

what should be a valid object and where should be either a valid object or `#-1` (denoting a location of 'nowhere'); otherwise `E_INVARG` is raised. The programmer must be either the owner of what or a wizard; otherwise, `E_PERM` is raised.

If where is a valid object, then the verb-call

```
where:accept(what)
```

is performed before any movement takes place. If the verb returns a false value and the programmer is not a wizard, then where is considered to have refused entrance to what; `move()` raises `E_NACC`. If where does not define an `accept` verb, then it is treated as if it defined one that always returned false.

If moving what into where would create a loop in the containment hierarchy (i.e., what would contain itself, even indirectly), then `E_RECMOVE` is raised instead.

The `location` property of what is changed to be where, and the `contents` properties of the old and new locations are modified appropriately. Let old-where be the location of what before it was moved. If old-where is a valid object, then the verb-call

```
old-where:exitfunc(what)
```

is performed and its result is ignored; it is not an error if old-where does not define a verb named `exitfunc`. Finally, if where and what are still valid objects, and where is still the location of what, then the verb-call

```
where:enterfunc(what)
```

is performed and its result is ignored; again, it is not an error if where does not define a verb named `enterfunc`.

Passing `position` into move will effectively listinsert() the object into that position in the .contents list.

##### Operations on Properties

**Function: `properties`**

properties -- Returns a list of the names of the properties defined directly on the given object, not inherited from its parent.

list `properties` (obj object)

If object is not valid, then `E_INVARG` is raised. If the programmer does not have read permission on object, then `E_PERM` is raised.

**Function: `property_info`**

property_info -- Get the owner and permission bits for the property named prop-name on the given object

list `property_info` (obj object, str prop-name)

If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then `E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then `property_info()` raises `E_PERM`.

**Function: `set_property_info`**

set_property_info -- Set the owner and permission bits for the property named prop-name on the given object

none `set_property_info` (obj object, str prop-name, list info)

If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then `E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then `set_property_info()` raises `E_PERM`. Property info has the following form:

```
{owner, perms [, new-name]}
```

where owner is an object, perms is a string containing only characters from the set `r`, `w`, and `c`, and new-name is a string; new-name is never part of the value returned by `property_info()`, but it may optionally be given as part of the value provided to `set_property_info()`. This list is the kind of value returned by property_info() and expected as the third argument to `set_property_info()`; the latter function raises `E_INVARG` if owner is not valid, if perms contains any illegal characters, or, when new-name is given, if prop-name is not defined directly on object or new-name names an existing property defined on object or any of its ancestors or descendants.

**Function: `add_property`**

add_property -- Defines a new property on the given object

none `add_property` (obj object, str prop-name, value, list info)

The property is inherited by all of its descendants; the property is named prop-name, its initial value is value, and its owner and initial permission bits are given by info in the same format as is returned by `property_info()`, described above.

If object is not valid or info does not specify a valid owner and well-formed permission bits or object or its ancestors or descendants already defines a property named prop-name, then `E_INVARG` is raised. If the programmer does not have write permission on object or if the owner specified by info is not the programmer and the programmer is not a wizard, then `E_PERM` is raised.

**Function: `delete_property`**

delete_property -- Removes the property named prop-name from the given object and all of its descendants.

none `delete_property` (obj object, str prop-name)

If object is not valid, then `E_INVARG` is raised. If the programmer does not have write permission on object, then `E_PERM` is raised. If object does not directly define a property named prop-name (as opposed to inheriting one from its parent), then `E_PROPNF` is raised.

**Function: `is_clear_property`**

is_clear_property -- Test the specified property for clear

int `is_clear_property` (obj object, str prop-name) **Function: `clear_property`**

clear_property -- Set the specified property to clear

none `clear_property` (obj object, str prop-name)

These two functions test for clear and set to clear, respectively, the property named prop-name on the given object. If object is not valid, then `E_INVARG` is raised. If object has no non-built-in property named prop-name, then `E_PROPNF` is raised. If the programmer does not have read (write) permission on the property in question, then `is_clear_property()` (`clear_property()`) raises `E_PERM`.

If a property is clear, then when the value of that property is queried the value of the parent's property of the same name is returned. If the parent's property is clear, then the parent's parent's value is examined, and so on.  If object is the definer of the property prop-name, as opposed to an inheritor of the property, then `clear_property()` raises `E_INVARG`.

##### Operations on Verbs

**Function: `verbs`**

verbs -- Returns a list of the names of the verbs defined directly on the given object, not inherited from its parent

list verbs (obj object)

If object is not valid, then `E_INVARG` is raised. If the programmer does not have read permission on object, then `E_PERM` is raised.

Most of the remaining operations on verbs accept a string containing the verb's name to identify the verb in question. Because verbs can have multiple names and because an object can have multiple verbs with the same name, this practice can lead to difficulties. To most unambiguously refer to a particular verb, one can instead use a positive integer, the index of the verb in the list returned by `verbs()`, described above.

For example, suppose that `verbs(#34)` returns this list:

```
{"foo", "bar", "baz", "foo"}
```

Object `#34` has two verbs named `foo` defined on it (this may not be an error, if the two verbs have different command syntaxes). To refer unambiguously to the first one in the list, one uses the integer 1; to refer to the other one, one uses 4.

In the function descriptions below, an argument named verb-desc is either a string containing the name of a verb or else a positive integer giving the index of that verb in its defining object's `verbs()` list.
For historical reasons, there is also a second, inferior mechanism for referring to verbs with numbers, but its use is strongly discouraged. If the property `$server_options.support_numeric_verbname_strings` exists with a true value, then functions on verbs will also accept a numeric string (e.g., `"4"`) as a verb descriptor. The decimal integer in the string works more-or-less like the positive integers described above, but with two significant differences:

The numeric string is a _zero-based_ index into `verbs()`; that is, in the string case, you would use the number one less than what you would use in the positive integer case.

When there exists a verb whose actual name looks like a decimal integer, this numeric-string notation is ambiguous; the server will in all cases assume that the reference is to the first verb in the list for which the given string could be a name, either in the normal sense or as a numeric index.

Clearly, this older mechanism is more difficult and risky to use; new code should only be written to use the current mechanism, and old code using numeric strings should be modified not to do so.

**Function: `verb_info`**

verb_info -- Get the owner, permission bits, and name(s) for the verb as specified by verb-desc on the given object

list `verb_info` (obj object, str|int verb-desc) 

**Function: `set_verb_info`**

set_verb_info -- Set the owner, permissions bits, and names(s) for the verb as verb-desc on the given object

none `set_verb_info` (obj object, str|int verb-desc, list info)

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read (write) permission on the verb in question, then `verb_info()` (`set_verb_info()`) raises `E_PERM`.

Verb info has the following form:

```
{owner, perms, names}
```

where owner is an object, perms is a string containing only characters from the set `r`, `w`, `x`, and `d`, and names is a string. This is the kind of value returned by `verb_info()` and expected as the third argument to `set_verb_info()`. `set_verb_info()` raises `E_INVARG` if owner is not valid, if perms contains any illegal characters, or if names is the empty string or consists entirely of spaces; it raises `E_PERM` if owner is not the programmer and the programmer is not a wizard.

**Function: `verb_args`**

verb_args -- get the direct-object, preposition, and indirect-object specifications for the verb as specified by verb-desc on the given object.

list `verb_args` (obj object, str|int verb-desc) 

**Function: `set_verb_args`**

verb_args -- set the direct-object, preposition, and indirect-object specifications for the verb as specified by verb-desc on the given object.

none `set_verb_args` (obj object, str|int verb-desc, list args)

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read (write) permission on the verb in question, then the function raises `E_PERM`.

Verb args specifications have the following form:

```
{dobj, prep, iobj}
```

where dobj and iobj are strings drawn from the set `"this"`, `"none"`, and `"any"`, and prep is a string that is either `"none"`, `"any"`, or one of the prepositional phrases listed much earlier in the description of verbs in the first chapter. This is the kind of value returned by `verb_args()` and expected as the third argument to `set_verb_args()`. Note that for `set_verb_args()`, prep must be only one of the prepositional phrases, not (as is shown in that table) a set of such phrases separated by `/` characters. `set_verb_args` raises `E_INVARG` if any of the dobj, prep, or iobj strings is illegal.

```
verb_args($container, "take")
                    =>   {"any", "out of/from inside/from", "this"}
set_verb_args($container, "take", {"any", "from", "this"})
```

**Function: `add_verb`**

add_verb -- defines a new verb on the given object

none `add_verb` (obj object, list info, list args)

The new verb's owner, permission bits and name(s) are given by info in the same format as is returned by `verb_info()`, described above. The new verb's direct-object, preposition, and indirect-object specifications are given by args in the same format as is returned by `verb_args`, described above. The new verb initially has the empty program associated with it; this program does nothing but return an unspecified value.

If object is not valid, or info does not specify a valid owner and well-formed permission bits and verb names, or args is not a legitimate syntax specification, then `E_INVARG` is raised. If the programmer does not have write permission on object or if the owner specified by info is not the programmer and the programmer is not a wizard, then `E_PERM` is raised.

**Function: `delete_verb`**

delete_verb -- removes the verb as specified by verb-desc from the given object

none `delete_verb` (obj object, str|int verb-desc)

If object is not valid, then `E_INVARG` is raised. If the programmer does not have write permission on object, then `E_PERM` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised.

**Function: `verb_code`**

verb_code -- get the MOO-code program associated with the verb as specified by verb-desc on object

list `verb_code` (obj object, str|int verb-desc [, fully-paren [, indent]]) 

**Function: `set_verb_code`**

set_verb_code -- set the MOO-code program associated with the verb as specified by verb-desc on object

list `set_verb_code` (obj object, str|int verb-desc, list code)

The program is represented as a list of strings, one for each line of the program; this is the kind of value returned by `verb_code()` and expected as the third argument to `set_verb_code()`. For `verb_code()`, the expressions in the returned code are usually written with the minimum-necessary parenthesization; if full-paren is true, then all expressions are fully parenthesized.

Also for `verb_code()`, the lines in the returned code are usually not indented at all; if indent is true, each line is indented to better show the nesting of statements.

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read (write) permission on the verb in question, then `verb_code()` (`set_verb_code()`) raises `E_PERM`. If the programmer is not, in fact. a programmer, then `E_PERM` is raised.

For `set_verb_code()`, the result is a list of strings, the error messages generated by the MOO-code compiler during processing of code. If the list is non-empty, then `set_verb_code()` did not install code; the program associated with the verb in question is unchanged.

**Function: `disassemble`**

disassemble -- returns a (longish) list of strings giving a listing of the server's internal "compiled" form of the verb as specified by verb-desc on object

list `disassemble` (obj object, str|int verb-desc)

This format is not documented and may indeed change from release to release, but some programmers may nonetheless find the output of `disassemble()` interesting to peruse as a way to gain a deeper appreciation of how the server works.

If object is not valid, then `E_INVARG` is raised. If object does not define a verb as specified by verb-desc, then `E_VERBNF` is raised. If the programmer does not have read permission on the verb in question, then `disassemble()` raises `E_PERM`.

##### Operations on WAIFs

**Function: `new_waif`**

new_waif -- The `new_waif()` builtin creates a new WAIF whose class is the calling object and whose owner is the perms of the calling verb.

waif `new_waif`()

This wizardly version causes it to be owned by the caller of the verb.

**Function: `waif_stats`**

waif_stats -- Returns a MAP of statistics about instantiated waifs.

map `waif_stats`()

Each waif class will be a key in the MAP and its value will be the number of waifs of that class currently instantiated. Additionally, there is a `total' key that will return the total number of instantiated waifs, and a `pending_recycle' key that will return the number of waifs that have been destroyed and are awaiting the call of their :recycle verb.

##### Operations on Player Objects

**Function: `players`**

players -- returns a list of the object numbers of all player objects in the database

list `players` ()

**Function: `is_player`**

is_player -- returns a true value if the given object is a player object and a false value otherwise.

int `is_player` (obj object)

If object is not valid, `E_INVARG` is raised.

**Function: `set_player_flag`**

set_player_flag -- confers or removes the "player object" status of the given object, depending upon the truth value of value

none `set_player_flag` (obj object, value)

If object is not valid, `E_INVARG` is raised. If the programmer is not a wizard, then `E_PERM` is raised.

If value is true, then object gains (or keeps) "player object" status: it will be an element of the list returned by `players()`, the expression `is_player(object)` will return true, and the server will treat a call to `$do_login_command()` that returns object as logging in the current connection.

If value is false, the object loses (or continues to lack) "player object" status: it will not be an element of the list returned by `players()`, the expression `is_player(object)` will return false, and users cannot connect to object by name when they log into the server. In addition, if a user is connected to object at the time that it loses "player object" status, then that connection is immediately broken, just as if `boot_player(object)` had been called (see the description of `boot_player()` below).

#### Operations on Files

There are several administrator-only builtins for manipulating files from inside the MOO.  Security is enforced by making these builtins executable with wizard permissions only as well as only allowing access to a directory under the current directory (the one the server is running in). The new builtins are structured similarly to the stdio library for C. This allows MOO-code to perform stream-oriented I/O to files.

Granting MOO code direct access to files opens a hole in the otherwise fairly good wall that the ToastStunt server puts up between the OS and the database.  The security is fairly well mitigated by restricting where files can be opened and allowing the builtins to be called by wizard permissions only. It is still possible execute various forms denial of service attacks, but the MOO server allows this form of attack as well.

> Warning: Depending on what Core you are using (ToastCore, LambdaMOO, etc) you may have a utility that acts as a wrapper around the FileIO code. This is the preferred method for dealing with files and directly using the built-ins is discouraged. On ToastCore you may have a $file WAIF you can utilize for this purpose.

> Warning: The FileIO code looks for a 'files' directory in the same directory as the MOO executable. This directory must exist for your code to work.

> Note: More detailed information regarding the FileIO code can be found in the docs/FileioDocs.txt folder of the ToastStunt repo.

The FileIO system has been updated in ToastCore and includes a number of enhancements over earlier LambdaMOO and Stunt versions.
* Faster reading
* Open as many files as you want, configurable with FILE_IO_MAX_FILES or $server_options.file_io_max_files

**FileIO Error Handling**

Errors are always handled by raising some kind of exception. The following exceptions are defined:

`E_FILE`

This is raised when a stdio call returned an error value. CODE is set to E_FILE, MSG is set to the return of strerror() (which may vary from system to system), and VALUE depends on which function raised the error.  When a function fails because the stdio function returned EOF, VALUE is set to "EOF".

`E_INVARG`

This is raised for a number of reasons.  The common reasons are an invalid FHANDLE being passed to a function and an invalid pathname specification.  In each of these cases MSG will be set to the cause and VALUE will be the offending value.

`E_PERM`

This is raised when any of these functions are called with non- wizardly permissions.

**General Functions**

**Function: `file_version`**

file_version -- Returns the package shortname/version number of this package e.g.

str `file_version`()

`file_version() => "FIO/1.7"`

**Opening and closing of files and related functions**

File streams are associated with FHANDLES.  FHANDLES are similar to the FILE\* using stdio.  You get an FHANDLE from file_open.  You should not depend on the actual type of FHANDLEs (currently TYPE_INT).  FHANDLEs are not persistent across server restarts.  That is, files open when the server is shut down are closed when it comes back up and no information about open files is saved in the DB.

**Function: `file_open`**

file_open -- Open a file 

FHANDLE `file_open`(STR pathname, STR mode)

Raises: E_INVARG if mode is not a valid mode, E_QUOTA if too many files are open.

This opens a file specified by pathname and returns an FHANDLE for it.  It ensures pathname is legal.  Mode is a string of characters indicating what mode the file is opened in. The mode string is four characters.

The first character must be (r)ead, (w)rite, or (a)ppend.  The second must be '+' or '-'.  This modifies the previous argument.

* r- opens the file for reading and fails if the file does not exist.
* r+ opens the file for reading and writing and fails if the file does not exist.
* w- opens the file for writing, truncating if it exists and creating if not.
* w+ opens the file for reading and writing, truncating if it exists and creating if not.
* a- opens a file for writing, creates it if it does not exist and positions the stream at the end of the file.
* a+ opens the file for reading and writing, creates it if does not exist and positions the stream at the end of the file.

The third character is either (t)ext or (b)inary.  In text mode, data is written as-is from the MOO and data read in by the MOO is stripped of unprintable characters.  In binary mode, data is written filtered through the binary-string->raw-bytes conversion and data is read filtered through the raw-bytes->binary-string conversion.  For example, in text mode writing " 1B" means three bytes are written: ' ' Similarly, in text mode reading " 1B" means the characters ' ' '1' 'B' were present in the file.  In binary mode reading " 1B" means an ASCII ESC was in the file.  In text mode, reading an ESC from a file results in the ESC getting stripped.

It is not recommended that files containing unprintable ASCII  data be read in text mode, for obvious reasons.

The final character is either 'n' or 'f'.  If this character is 'f', whenever data is written to the file, the MOO will force it to finish writing to the physical disk before returning.  If it is 'n' then this won't happen.

This is implemented using fopen().

** Function: `file_close`**

file_close -- Close a file 

void `file_close`(FHANDLE fh)

Closes the file associated with fh.

This is implemented using fclose().

** Function: `file_name`**

file_name -- Returns the pathname originally associated with fh by file_open().  This is not necessarily the file's current name if it was renamed or unlinked after the fh was opened.

STR `file_name`(FHANDLE fh)

** Function: `file_openmode`**

file_open_mode -- Returns the mode the file associated with fh was opened in.

str `file_openmode`(FHANDLE fh)

** Function: `file_handles`**

file_handles -- Return a list of open files

LIST `file_handles` ()

**Input and Output Operations**

** Function: `file_readline`**

file_readline -- Reads the next line in the file and returns it (without the newline).  

str `file_readline`(FHANDLE fh)

Not recommended for use on files in binary mode.

This is implemented using fgetc().

** Function: `file_readlines`**

file_readlines -- Rewinds the file and then reads the specified lines from the file, returning them as a list of strings.  After this operation, the stream is positioned right after the last line read.

list `file_readlines`(FHANDLE fh, INT start, INT end)

Not recommended for use on files in binary mode.

This is implemented using fgetc().

** Function: `file_writeline`**

file_writeline -- Writes the specified line to the file (adding a newline).

void `file_writeline`(FHANDLE fh, STR line)

Not recommended for use on files in binary mode.

This is implemented using fputs()

** Function: `file_read`**

file_read -- Reads up to the specified number of bytes from the file and returns them.

str `file_read`(FHANDLE fh, INT bytes)

Not recommended for use on files in text mode.

This is implemented using fread().

** Function: `file_write`**

file_write -- Writes the specified data to the file. Returns number of bytes written.

int `file_write`(FHANDLE fh, STR data)

Not recommended for use on files in text mode.

This is implemented using fwrite().

** Function: `file_count_lines`**

file_count_lines -- count the lines in a file

INT `file_count_lines` (FHANDLER fh)

** Function: `file_grep`**

file_grep -- search for a string in a file

LIST `file_grep`(FHANDLER fh, STR search [,?match_all = 0])

Assume we have a file `test.txt` with the contents:

```
asdf asdf 11
11
112
```

And we have an open file handler from running:

```
;file_open("test.txt", "r-tn")
```

If we were to execute a file grep:

```
;file_grep(1, "11")
```

We would get the first result:

```
{{"asdf asdf 11", 1}}
```

The resulting LIST is of the form {{STR match, INT line-number}}

If you pass in the optional third argument

```
;file_grep(1, "11", 1)
```

we will receive all the matching results:

```
{{"asdf asdf 11", 1}, {"11", 2}, {"112", 3}}
```

**Getting and setting stream position**

** Function: `file_tell`**

file_tell -- Returns position in file.

INT `file_tell`(FHANDLE fh)

This is implemented using ftell().

** Function: `file_seek`**

file_seek -- Seeks to a particular location in a file.  

void `file_seek`(FHANDLE fh, INT loc, STR whence)

whence is one of the strings:

* "SEEK_SET" - seek to location relative to beginning
* "SEEK_CUR" - seek to location relative to current
* "SEEK_END" - seek to location relative to end

This is implemented using fseek().

** Function: `file_eof`**

file_eof -- Returns true if and only if fh's stream is positioned at EOF.

int `file_eof`(FHANDLE fh)

This is implemented using feof().

**Housekeeping operations**

** Function: `file_size`**

** Function: `file_last_access`**

** Function: `file_last_modify`**

** Function: `file_last_change`**

** Function: `file_size`**

int `file_size`(STR pathname)

int `file_last_access`(STR pathname)

int `file_last_modify`(STR pathname)

int `file_last_change`(STR pathname)

int `file_size`(FHANDLE filehandle)

int `file_last_access`(FHANDLE filehandle)

int `file_last_modify`(FHANDLE filehandle)

int `file_last_change`(FHANDLE filehandle)

Returns the size, last access time, last modify time, or last change time of the specified file.   All of these functions also take FHANDLE arguments and then operate on the open file.

** Function: `file_mode`**

int `file_mode`(STR filename)

int `file_mode`(FHANDLE fh)

Returns octal mode for a file (e.g. "644").

This is implemented using stat().

**file_stat**

void `file_stat`(STR pathname)

void `file_stat`(FHANDLE fh)

Returns the result of stat() (or fstat()) on the given file.

Specifically a list as follows:

`{file size in bytes, file type, file access mode, owner, group, last access, last modify, and last change}`

owner and group are always the empty string.

It is recommended that the specific information functions file_size, file_type, file_mode, file_last_access, file_last_modify, and file_last_change be used instead.  In most cases only one of these elements is desired and in those cases there's no reason to make and free a list.

** Function: `file_rename`**

file_rename - Attempts to rename the oldpath to newpath.

void `file_rename`(STR oldpath, STR newpath)

This is implemented using rename().

**file_remove**

file_remove -- Attempts to remove the given file.
 
void `file_remove`(STR pathname)

This is implemented using remove().

**Function: `file_mkdir`**

file_mkdir -- Attempts to create the given directory.

void `file_mkdir`(STR pathname)

This is implemented using mkdir().

**Function: `file_rmdir`**

file_rmdir -- Attempts to remove the given directory.

void `file_rmdir`(STR pathname)

This is implemented using rmdir().

**Function: `file_list`**

file_list -- Attempts to list the contents of the given directory.

LIST `file_list`(STR pathname, [ANY detailed])

Returns a list of files in the directory.  If the detailed argument is provided and true, then the list contains detailed entries, otherwise it contains a simple list of names.

detailed entry:

`{STR filename, STR file type, STR file mode, INT file size}`

normal entry:

STR filename

This is implemented using scandir().

**Function: `file_type`**

file_type -- Returns the type of the given pathname, one of "reg", "dir", "dev", "fifo", or "socket".

STR `file_type`(STR pathname)

This is implemented using stat().

**Function: `file_chmod`**

file_chmod -- Attempts to set mode of a file using mode as an octal string of exactly three characters.

void `file_chmod`(STR filename, STR mode)

This is implemented using chmod().

##### Operations on SQLite

SQLite allows you to store information in locally hosted SQLite databases.

**Function: `sqlite_open`**

sqlite_open -- The function `sqlite_open` will attempt to open the database at path for use with SQLite.

int `sqlite_open`(STR path to database, [INT options])

The second argument is a bitmask of options. Options are:

SQLITE_PARSE_OBJECTS [4]:    Determines whether strings beginning with a pound symbol (#) are interpreted as MOO object numbers or not. The default is true, which means that any queries that would return a string (such as "#123") will be returned as objects.

SQLITE_PARSE_TYPES [2]:      If unset, no parsing of rows takes place and only strings are returned.

SQLITE_SANITIZE_STRINGS [8]: If set, newlines (\n) are converted into tabs (\t) to avoid corrupting the MOO database. Default is unset.

> Note: If the MOO doesn't support bitmasking, you can still specify options. You'll just have to manipulate the int yourself. e.g. if you want to parse objects and types, arg[2] would be a 6. If you only want to parse types, arg[2] would be 2.

If successful, the function will return the numeric handle for the open database.

If unsuccessful, the function will return a helpful error message.

If the database is already open, a traceback will be thrown that contains the already open database handle.

**Function: `sqlite_close`**

sqlite_close -- This function will close an open database.

int `sqlite_close`(INT database handle)

If successful, return 1;

If unsuccessful, returns E_INVARG.

**Function: `sqlite_execute`**

sqlite_execute -- This function will attempt to create and execute the prepared statement query given in query on the database referred to by handle with the values values.

list | str `sqlite_execute`(INT database handle, STR SQL prepared statement query, LIST values)

On success, this function will return a list identifying the returned rows. If the query didn't return rows but was successful, an empty list is returned.

If the query fails, a string will be returned identifying the SQLite error message.

`sqlite_execute` uses prepared statements, so it's the preferred function to use for security and performance reasons.

Example:

```
sqlite_execute(0, "INSERT INTO users VALUES (?, ?, ?);", {#7, "lisdude", "Albori Sninvel"})
```

ToastStunt supports the REGEXP pattern matching operator:

```
sqlite_execute(4, "SELECT rowid FROM notes WHERE body REGEXP ?;", {"albori (sninvel)?"})
```

> Note: This is a threaded function.

**Function: `sqlite_query`**

sqlite_query -- This function will attempt to execute the query given in query on the database referred to by handle.

list | str `sqlite_query`(INT database handle, STR database query[, INT show columns])

On success, this function will return a list identifying the returned rows. If the query didn't return rows but was successful, an empty list is returned.

If the query fails, a string will be returned identifying the SQLite error message.

If show columns is true, the return list will include the name of the column before its results.

> Warning: sqlite_query does NOT use prepared statements and should NOT be used on queries that contain user input.

> Note: This is a threaded function.

**Function: `sqlite_limit`**

sqlite_limit -- This function allows you to specify various construct limitations on a per-database basis.

int `sqlite_limit`(INT database handle, STR category INT new value)

If new value is a negative number, the limit is unchanged. Each limit category has a hardcoded upper bound. Attempts to increase a limit above its hard upper bound are silently truncated to the hard upper bound.

Regardless of whether or not the limit was changed, the sqlite_limit() function returns the prior value of the limit. Hence, to find the current value of a limit without changing it, simply invoke this interface with the third parameter set to -1.

As of this writing, the following limits exist:

| Limit                     | Description                                                                                                                                                                                                                                                              |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| LIMIT_LENGTH              | The maximum size of any string or BLOB or table row, in bytes.                                                                                                                                                                                                           |
| LIMIT_SQL_LENGTH          | The maximum length of an SQL statement, in bytes.                                                                                                                                                                                                                        |
| LIMIT_COLUMN              | The maximum number of columns in a table definition or in the result set of a SELECT or the maximum number of columns in an index or in an ORDER BY or GROUP BY clause.                                                                                                  |
| LIMIT_EXPR_DEPTH          | The maximum depth of the parse tree on any expression.                                                                                                                                                                                                                   |
| LIMIT_COMPOUND_SELECT     | The maximum number of terms in a compound SELECT statement.                                                                                                                                                                                                              |
| LIMIT_VDBE_OP             | The maximum number of instructions in a virtual machine program used to implement an SQL statement. If sqlite3_prepare_v2() or the equivalent tries to allocate space for more than this many opcodes in a single prepared statement, an SQLITE_NOMEM error is returned. |
| LIMIT_FUNCTION_ARG        | The maximum number of arguments on a function.                                                                                                                                                                                                                           |
| LIMIT_ATTACHED            | The maximum number of attached databases.                                                                                                                                                                                                                                |
| LIMIT_LIKE_PATTERN_LENGTH | The maximum length of the pattern argument to the LIKE or GLOB operators.                                                                                                                                                                                                |
| LIMIT_VARIABLE_NUMBER     | The maximum index number of any parameter in an SQL statement.                                                                                                                                                                                                           |
| LIMIT_TRIGGER_DEPTH       | The maximum depth of recursion for triggers.                                                                                                                                                                                                                             |
| LIMIT_WORKER_THREADS | The maximum number of auxiliary worker threads that a single prepared statement may start. |

For an up-to-date list of limits, see the [SQLite documentation](https://www.sqlite.org/c3ref/c_limit_attached.html).

**Function: `sqlite_last_insert_row_id`**

sqlite_last_insert_row_id -- This function identifies the row ID of the last insert command executed on the database.

int `sqlite_last_insert_row_id`(INT database handle)

**Function: `sqlite_interrupt`**

sqlite_interrupt -- This function causes any pending database operation to abort at its earliest opportunity.

none `sqlite_interrupt`(INT database handle)

If the operation is nearly finished when sqlite_interrupt is called, it might not have an opportunity to be interrupted and could continue to completion.

This can be useful when you execute a long-running query and want to abort it.

> NOTE: As of this writing (server version 2.7.0) the @kill command WILL NOT abort operations taking place in a helper thread. If you want to interrupt an SQLite query, you must use sqlite_interrupt and NOT the @kill command.

**Function: `sqlite_info`**

sqlite_info -- This function returns a map of information about the database at handle

map `sqlite_info`(INT database handle)

The information returned is:

* Database Path
* Type parsing enabled?
* Object parsing enabled?
* String sanitation enabled?

**Function: `sqlite_handles`**

sqlite_handles -- Returns a list of open SQLite database handles.

list `sqlite_handles()`

##### Operations on The Server Environment

**Function: `exec`**

exec -- Asynchronously executes the specified external executable, optionally sending input.

list `exec` (list command[, str input])

Returns the process return code, output and error. If the programmer is not a wizard, then E_PERM is raised.

The first argument must be a list of strings, or E_INVARG is raised. The first string is the path to the executable and is required. The rest are command line arguments passed to the executable.

The path to the executable may not start with a slash (/) or dot-dot (..), and it may not contain slash-dot (/.) or dot-slash (./), or E_INVARG is raised. If the specified executable does not exist or is not a regular file, E_INVARG is raised.

If the string input is present, it is written to standard input of the executing process.

When the process exits, it returns a list of the form:

`{code, output, error}`

code is the integer process exit status or return code. output and error are strings of data that were written to the standard output and error of the process.

The specified command is executed asynchronously. The function suspends the current task and allows other tasks to run until the command finishes. Tasks suspended this way can be killed with kill_task().

The strings, input, output and error are all MOO binary strings.

All external executables must reside in the executables directory.

```
exec({"cat", "-?"})                                   ⇒   {1, "", "cat: illegal option -- ?~0Ausage: cat [-benstuv] [file ...]~0A"}
exec({"cat"}, "foo")                                  ⇒   {0, "foo", ""}
exec({"echo", "one", "two"})                          ⇒   {0, "one two~0A", ""}
```

You are able to set environmental variables with `exec`, imagine you had a `vars.sh` (in your executables directory):

```
#!/bin/bash
echo "pizza = ${pizza}"
```

And then you did:

```
exec({"vars.sh"}, "", {"pizza=tasty"}) => {0, "pizza = tasty~0A", ""}
exec({"vars.sh"}) => {0, "pizza = ~0A", ""}
```

The second time pizza doesn't exist. The darkest timeline.

**Function: `getenv`**

getenv -- Returns the value of the named environment variable. 

str `getenv` (str name)

If no such environment variable exists, 0 is returned. If the programmer is not a wizard, then E_PERM is raised.

```
getenv("HOME")                                          ⇒   "/home/foobar"
getenv("XYZZY")      
```

##### Operations on Network Connections

**Function: `connected_players`**

connected_players -- returns a list of the object numbers of those player objects with currently-active connections

list `connected_players` ([include-all])

If include-all is provided and true, then the list includes the object numbers associated with _all_ current connections, including ones that are outbound and/or not yet logged-in.

**Function: `connected_seconds`**

connected_seconds -- return the number of seconds that the currently-active connection to player has existed

int `connected_seconds` (obj player) **Function: `idle_seconds`**

idle_seconds -- return the number of seconds that the currently-active connection to player has been idle

int `idle_seconds` (obj player)

If player is not the object number of a player object with a currently-active connection, then `E_INVARG` is raised.

**Function: `notify`**

notify -- enqueues string for output (on a line by itself) on the connection conn

none `notify` (obj conn, str string [, INT no-flush [, INT suppress-newline])

If the programmer is not conn or a wizard, then `E_PERM` is raised. If conn is not a currently-active connection, then this function does nothing. Output is normally written to connections only between tasks, not during execution.

The server will not queue an arbitrary amount of output for a connection; the `MAX_QUEUED_OUTPUT` compilation option (in `options.h`) controls the limit (`MAX_QUEUED_OUTPUT` can be overridden in-database by adding the property `$server_options.max_queued_output` and calling `load_server_options()`). When an attempt is made to enqueue output that would take the server over its limit, it first tries to write as much output as possible to the connection without having to wait for the other end. If that doesn't result in the new output being able to fit in the queue, the server starts throwing away the oldest lines in the queue until the new output will fit. The server remembers how many lines of output it has 'flushed' in this way and, when next it can succeed in writing anything to the connection, it first writes a line like `>> Network buffer overflow: X lines of output to you have been lost <<` where X is the number of flushed lines.

If no-flush is provided and true, then `notify()` never flushes any output from the queue; instead it immediately returns false. `Notify()` otherwise always returns true.

If suppress-newline is provided and true, then `notify()` does not add a newline add the end of the string.

**Function: `buffered_output_length`**

buffered_output_length -- returns the number of bytes currently buffered for output to the connection conn

int `buffered_output_length` ([obj conn])

If conn is not provided, returns the maximum number of bytes that will be buffered up for output on any connection.

**Function: `read`**

read -- reads and returns a line of input from the connection conn (or, if not provided, from the player that typed the command that initiated the current task)

str `read` ([obj conn [, non-blocking]])

If non-blocking is false or not provided, this function suspends the current task, resuming it when there is input available to be read. If non-blocking is provided and true, this function never suspends the calling task; if there is no input currently available for input, `read()` simply returns 0 immediately.

If player is provided, then the programmer must either be a wizard or the owner of `player`; if `player` is not provided, then `read()` may only be called by a wizard and only in the task that was last spawned by a command from the connection in question. Otherwise, `E_PERM` is raised.

If the given `player` is not currently connected and has no pending lines of input, or if the connection is closed while a task is waiting for input but before any lines of input are received, then `read()` raises `E_INVARG`.

The restriction on the use of `read()` without any arguments preserves the following simple invariant: if input is being read from a player, it is for the task started by the last command that player typed. This invariant adds responsibility to the programmer, however. If your program calls another verb before doing a `read()`, then either that verb must not suspend or else you must arrange that no commands will be read from the connection in the meantime. The most straightforward way to do this is to call

```
set_connection_option(player, "hold-input", 1)
```

before any task suspension could happen, then make all of your calls to `read()` and other code that might suspend, and finally call

```
set_connection_option(player, "hold-input", 0)
```

to allow commands once again to be read and interpreted normally.

**Function: `force_input`**

force_input -- inserts the string line as an input task in the queue for the connection conn, just as if it had arrived as input over the network

none `force_input` (obj conn, str line [, at-front])

If at_front is provided and true, then the new line of input is put at the front of conn's queue, so that it will be the very next line of input processed even if there is already some other input in that queue. Raises `E_INVARG` if conn does not specify a current connection and `E_PERM` if the programmer is neither conn nor a wizard.

**Function: `flush_input`**

flush_input -- performs the same actions as if the connection conn's defined flush command had been received on that connection

none `flush_input` (obj conn [show-messages])

I.E., removes all pending lines of input from conn's queue and, if show-messages is provided and true, prints a message to conn listing the flushed lines, if any. See the chapter on server assumptions about the database for more information about a connection's defined flush command.

**Function: `output_delimiters`**

output_delimiters -- returns a list of two strings, the current _output prefix_ and _output suffix_ for player.

list `output_delimiters` (obj player)

If player does not have an active network connection, then `E_INVARG` is raised. If either string is currently undefined, the value `""` is used instead. See the discussion of the `PREFIX` and `SUFFIX` commands in the next chapter for more information about the output prefix and suffix.

**Function: `boot_player`**

boot_player -- marks for disconnection any currently-active connection to the given player

none `boot_player` (obj player)

The connection will not actually be closed until the currently-running task returns or suspends, but all MOO functions (such as `notify()`, `connected_players()`, and the like) immediately behave as if the connection no longer exists. If the programmer is not either a wizard or the same as player, then `E_PERM` is raised. If there is no currently-active connection to player, then this function does nothing.

If there was a currently-active connection, then the following verb call is made when the connection is actually closed:

```
$user_disconnected(player)
```

It is not an error if this verb does not exist; the call is simply skipped.

**Function: `connection_info`**

connection_info -- Returns a MAP of network connection information for `connection`. At the time of writing, the following information is returned:

list `connection_info` (OBJ `connection`)

| Key                 | Value                                                                                                                                                                                          |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| destination_address | The hostname of the connection. For incoming connections, this is the hostname of the connected user. For outbound connections, this is the hostname of the outbound connection's destination. |
| destination_ip      | The unresolved numeric IP address of the connection.                                                                                                                                           |
| destination_port    | For incoming connections, this is the local port used to make the connection. For outbound connections, this is the port the connection was made to.                                           |
| source_address      | This is the hostname of the interface an incoming connection was made on. For outbound connections, this value is meaningless.                                                                 |
| source_ip           | The unresolved numeric IP address of the interface a connection was made on. For outbound connections, this value is meaningless.                                                              |
| source_port         | The local port a connection connected to. For outbound connections, this value is meaningless.                                                                                                 |
| protocol            | Describes the protocol used to make the connection. At the time of writing, this could be IPv4 or IPv6.                                                                                        |
| outbound | Indicates whether a connection is outbound or not |

**Function: `connection_name`**

connection_name -- returns a network-specific string identifying the connection being used by the given player

str `connection_name` (obj player, [INT method])

When provided just a player object this function only returns obj's hostname (e.g. `1-2-3-6.someplace.com`). An optional argument allows you to specify 1 if you want a numeric IP address, or 2 if you want to return the legacy connection_name string.

> Warning: If you are using a LambdaMOO core, this is a semi-breaking change. You'll want to update any code on your server that runs `connection_name` to pass in the argument for returning the legacy connection_name string if you want things to work exactly the same.

If the programmer is not a wizard and not player, then `E_PERM` is raised. If player is not currently connected, then `E_INVARG` is raised.

Legacy Connection String Information:

For the TCP/IP networking configurations, for in-bound connections, the string has the form:

```
"port lport from host, port port"
```

where lport is the decimal TCP listening port on which the connection arrived, host is either the name or decimal TCP address of the host from which the player is connected, and port is the decimal TCP port of the connection on that host.

For outbound TCP/IP connections, the string has the form

```
"port lport to host, port port"
```

where lport is the decimal local TCP port number from which the connection originated, host is either the name or decimal TCP address of the host to which the connection was opened, and port is the decimal TCP port of the connection on that host.

For the System V 'local' networking configuration, the string is the UNIX login name of the connecting user or, if no such name can be found, something of the form:

```
"User #number"
```

where number is a UNIX numeric user ID.

For the other networking configurations, the string is the same for all connections and, thus, useless.

**Function: `connection_name_lookup`**

connection_name_lookup - This function performs a DNS name lookup on connection's IP address.

str `connection_name_lookup` (OBJ connection [, INT record_result])

If a hostname can't be resolved, the function simply returns the numeric IP address. Otherwise, it will return the resolved hostname.

If record_result is true, the resolved hostname will be saved with the connection and will overwrite it's existing 'connection_name()'. This means that you can call 'connection_name_lookup()' a single time when a connection is created and then continue to use 'connection_name()' as you always have in the past.

This function is primarily intended for use when the 'NO_NAME_LOOKUP' server option is set. Barring temporarily failures in your nameserver, very little will be gained by calling this when the server is performing DNS name lookups for you.

> Note: This function runs in a separate thread. While this is good for performance (long lookups won't lock your MOO like traditional pre-2.6.0 name lookups), it also means it will require slightly more work to create an entirely in-database DNS lookup solution. Because it explicitly suspends, you won't be able to use it in 'do_login_command()' without also using the 'switch_player()' function. For an example of how this can work, see '#0:do_login_command()' in ToastCore.

**Function: `switch_player`**

switch_player -- Silently switches the player associated with this connection from object1 to object2.

`switch_player`(OBJ object1, OBJ object2 [, INT silent])

object1 must be connected and object2 must be a player. This can be used in do_login_command() verbs that read or suspend (which prevents the normal player selection mechanism from working.

If silent is true, no connection messages will be printed.

> Note: This calls the listening object's user_disconnected and user_connected verbs when appropriate.

**Function: `set_connection_option`**

set_connection_option -- controls a number of optional behaviors associated the connection conn

none `set_connection_option` (obj conn, str option, value)

Raises E_INVARG if conn does not specify a current connection and E_PERM if the programmer is neither conn nor a wizard. Unless otherwise specified below, options can only be set (value is true) or unset (otherwise). The following values for option are currently supported: 

The following values for option are currently supported:

`"binary"`
When set, the connection is in binary mode, in which case both input from and output to conn can contain arbitrary bytes. Input from a connection in binary mode is not broken into lines at all; it is delivered to either the read() function or normal command parsing as binary strings, in whatever size chunks come back from the operating system. (See fine point on binary strings, for a description of the binary string representation.) For output to a connection in binary mode, the second argument to `notify()` must be a binary string; if it is malformed, E_INVARG is raised.

> Fine point: If the connection mode is changed at any time when there is pending input on the connection, said input will be delivered as per the previous mode (i.e., when switching out of binary mode, there may be pending “lines” containing tilde-escapes for embedded linebreaks, tabs, tildes and other characters; when switching into binary mode, there may be pending lines containing raw tabs and from which nonprintable characters have been silently dropped as per normal mode. Only during the initial invocation of $do_login_command() on an incoming connection or immediately after the call to open_network_connection() that creates an outgoing connection is there guaranteed not to be pending input. At other times you will probably want to flush any pending input immediately after changing the connection mode. 

`"hold-input"`

When set, no input received on conn will be treated as a command; instead, all input remains in the queue until retrieved by calls to read() or until this connection option is unset, at which point command processing resumes. Processing of out-of-band input lines is unaffected by this option. 

 `"disable-oob"`

When set, disables all out of band processing (see section Out-of-Band Processing). All subsequent input lines until the next command that unsets this option will be made available for reading tasks or normal command parsing exactly as if the out-of-band prefix and the out-of-band quoting prefix had not been defined for this server.

`"client-echo"`
The setting of this option is of no significance to the server. However calling set_connection_option() for this option sends the Telnet Protocol `WONT ECHO` or `WILL ECHO` according as value is true or false, respectively. For clients that support the Telnet Protocol, this should toggle whether or not the client echoes locally the characters typed by the user. Note that the server itself never echoes input characters under any circumstances. (This option is only available under the TCP/IP networking configurations.) 

`"flush-command"`
This option is string-valued. If the string is non-empty, then it is the flush command for this connection, by which the player can flush all queued input that has not yet been processed by the server. If the string is empty, then conn has no flush command at all. set_connection_option also allows specifying a non-string value which is equivalent to specifying the empty string. The default value of this option can be set via the property `$server_options.default_flush_command`; see Flushing Unprocessed Input for details. 

`"intrinsic-commands"`

This option value is a list of strings, each being the name of one of the available server intrinsic commands (see section Command Lines That Receive Special Treatment). Commands not on the list are disabled, i.e., treated as normal MOO commands to be handled by $do_command and/or the built-in command parser

set_connection_option also allows specifying an integer value which, if zero, is equivalent to specifying the empty list, and otherwise is taken to be the list of all available intrinsic commands (the default setting).

Thus, one way to make the verbname `PREFIX` available as an ordinary command is as follows:

```
set_connection_option(
  player, "intrinsic-commands",
  setremove(connection_options(player, "intrinsic-commands"),
            "PREFIX"));
```

Note that connection_options() with no second argument will return a list while passing in the second argument will return the value of the key requested.

```
save = connection_options(player,"intrinsic-commands");
set_connection_options(player, "intrinsic-commands, 1);
full_list = connection_options(player,"intrinsic-commands");
set_connection_options(player,"intrinsic-commands", save);
return full_list;
```

is a way of getting the full list of intrinsic commands available in the server while leaving the current connection unaffected. 

**Function: `connection_options`**

connection_options -- returns a list of `{name, value}` pairs describing the current settings of all of the allowed options for the connection conn or the value if `name` is provided

ANY `connection_options` (obj conn [, STR name])

Raises `E_INVARG` if conn does not specify a current connection and `E_PERM` if the programmer is neither conn nor a wizard.

Calling connection options without a name will return a LIST. Passing in name will return only the value for the option `name` requested.

**Function: `open_network_connection`**

open_network_connection -- establishes a network connection to the place specified by the arguments and more-or-less pretends that a new, normal player connection has been established from there

obj `open_network_connection` (STR host, INT port [, MAP options])

Establishes a network connection to the place specified by the arguments and more-or-less pretends that a new, normal player connection has been established from there.  The new connection, as usual, will not be logged in initially and will have a negative object number associated with it for use with `read()', `notify()', and `boot_player()'.  This object number is the value returned by this function.

If the programmer is not a wizard or if the `OUTBOUND_NETWORK' compilation option was not used in building the server, then `E_PERM' is raised.

`host` refers to a string naming a host (possibly a numeric IP address) and `port` is an integer referring to a TCP port number.  If a connection cannot be made because the host does not exist, the port does not exist, the host is not reachable or refused the connection, `E_INVARG' is raised.  If the connection cannot be made for other reasons, including resource limitations, then `E_QUOTA' is raised.

Optionally, you can specify a map with any or all of the following options:

  listener: An object whose listening verbs will be called at appropriate points. (See HELP LISTEN() for more details.)

  tls:      If true, establish a secure TLS connection.

  ipv6:     If true, utilize the IPv6 protocol rather than the IPv4 protocol.

The outbound connection process involves certain steps that can take quite a long time, during which the server is not doing anything else, including responding to user commands and executing MOO tasks.  See the chapter on server assumptions about the database for details about how the server limits the amount of time it will wait for these steps to successfully complete.

It is worth mentioning one tricky point concerning the use of this function.  Since the server treats the new connection pretty much like any normal player connection, it will naturally try to parse any input from that connection as commands in the usual way.  To prevent this treatment, you should use `set_connection_option()' to set the `hold-input' option true on the connection.

Example:

```
open_network_connection("2607:5300:60:4be0::", 1234, ["ipv6" -> 1, "listener" -> #6, "tls" -> 1])
```

Open a new connection to the IPv6 address 2607:5300:60:4be0:: on port 1234 using TLS. Relevant verbs will be called on #6.

**Function: `curl`**

str `curl`(STR url [, INT include_headers, [ INT timeout])

The curl builtin will download a webpage and return it as a string. If include_headers is true, the HTTP headers will be included in the return string.

It's worth noting that the data you get back will be binary encoded. In particular, you will find that line breaks appear as ~0A. You can easily convert a page into a list by passing the return string into the decode_binary() function.

CURL_TIMEOUT is defined in options.h to specify the maximum amount of time a CURL request can take before failing. For special circumstances, you can specify a longer or shorter timeout using the third argument of curl().

**Function: `read_http`**

map `read_http` (request-or-response [, OBJ conn])

Reads lines from the connection conn (or, if not provided, from the player that typed the command that initiated the current task) and attempts to parse the lines as if they are an HTTP request or response. request-or-response must be either the string "request" or "response". It dictates the type of parsing that will be done.

Just like read(), if conn is provided, then the programmer must either be a wizard or the owner of conn; if conn is not provided, then read_http() may only be called by a wizard and only in the task that was last spawned by a command from the connection in question. Otherwise, E_PERM is raised. Likewise, if conn is not currently connected and has no pending lines of input, or if the connection is closed while a task is waiting for input but before any lines of input are received, then read_http() raises E_INVARG.

If parsing fails because the request or response is syntactically incorrect, read_http() will return a map with the single key "error" and a list of values describing the reason for the error. If parsing succeeds, read_http() will return a map with an appropriate subset of the following keys, with values parsed from the HTTP request or response: "method", "uri", "headers", "body", "status" and "upgrade".

 > Fine point: read_http() assumes the input strings are binary strings. When called interactively, as in the example below, the programmer must insert the literal line terminators or parsing will fail. 

The following example interactively reads an HTTP request from the players connection.

```
read_http("request", player)
GET /path HTTP/1.1~0D~0A
Host: example.com~0D~0A
~0D~0A
```

In this example, the string ~0D~0A ends the request. The call returns the following (the request has no body):

```
["headers" -> ["Host" -> "example.com"], "method" -> "GET", "uri" -> "/path"]
```

The following example interactively reads an HTTP response from the players connection.

```
read_http("response", player)
HTTP/1.1 200 Ok~0D~0A
Content-Length: 10~0D~0A
~0D~0A
1234567890
```

The call returns the following:

```
["body" -> "1234567890", "headers" -> ["Content-Length" -> "10"], "status" -> 200]
```

**Function: `listen`**

listen -- create a new point at which the server will listen for network connections, just as it does normally

value `listen` (obj object, port [, MAP options])

Create a new point at which the server will listen for network connections, just as it does normally. `Object` is the object whose verbs `do_login_command', `do_command', `do_out_of_band_command', `user_connected', `user_created', `user_reconnected', `user_disconnected', and `user_client_disconnected' will be called at appropriate points as these verbs are called on #0 for normal connections. (See the chapter in the LambdaMOO Programmer's Manual on server assumptions about the database for the complete story on when these functions are called.) `Port` is a TCP port number on which to listen. The listen() function will return `port` unless `port` is zero, in which case the return value is a port number assigned by the operating system.

An optional third argument allows you to set various miscellaneous options for the listening point. These are:

  print-messages: If true, the various database-configurable messages (also detailed in the chapter on server assumptions) will be printed on connections received at the new listening port.

  ipv6:           Use the IPv6 protocol rather than IPv4.

  tls:            Only accept valid secure TLS connections.

  certificate:    The full path to a TLS certificate. NOTE: Requires the TLS option also be specified and true. This option is only necessary if the certificate differs from the one specified in options.h.

  key:            The full path to a TLS private key. NOTE: Requires the TLS option also be specified and true. This option is only necessary if the key differs from the one specified in options.h.

listen() raises E_PERM if the programmer is not a wizard, E_INVARG if `object` is invalid or there is already a listening point described by `point`, and E_QUOTA if some network-configuration-specific error occurred.

Example:

```
listen(#0, 1234, ["ipv6" -> 1, "tls" -> 1, "certificate" -> "/etc/certs/something.pem", "key" -> "/etc/certs/privkey.pem", "print-messages" -> 1]
```

Listen for IPv6 connections on port 1234 and print messages as appropriate. These connections must be TLS and will use the private key and certificate found in /etc/certs.

**Function: `unlisten`**

unlisten -- stop listening for connections on the point described by canon, which should be the second element of some element of the list returned by `listeners()`

none `unlisten` (canon)

Raises `E_PERM` if the programmer is not a wizard and `E_INVARG` if there does not exist a listener with that description.

**Function: `listeners`**

listeners -- returns a list describing all existing listening points, including the default one set up automatically by the server when it was started (unless that one has since been destroyed by a call to `unlisten()`)

list `listeners` ()

Each element of the list has the following form:

```
{object, canon, print-messages}
```

where object is the first argument given in the call to `listen()` to create this listening point, print-messages is true if the third argument in that call was provided and true, and canon was the value returned by that call. (For the initial listening point, object is `#0`, canon is determined by the command-line arguments or a network-configuration-specific default, and print-messages is true.)

Please note that there is nothing special about the initial listening point created by the server when it starts; you can use `unlisten()` on it just as if it had been created by `listen()`. This can be useful; for example, under one of the TCP/IP configurations, you might start up your server on some obscure port, say 12345, connect to it by yourself for a while, and then open it up to normal users by evaluating the statements:

```
unlisten(12345); listen(#0, 7777, 1)
```

##### Operations Involving Times and Dates

**Function: `time`**

time -- returns the current time, represented as the number of seconds that have elapsed since midnight on 1 January 1970, Greenwich Mean Time

int `time` ()

**Function: `ftime`**

ftime -- Returns the current time represented as the number of seconds and nanoseconds that have elapsed since midnight on 1 January 1970, Greenwich Mean Time.

float `ftime` ([INT monotonic])

If the `monotonic` argument is supplied and set to 1, the time returned will be monotonic. This means that will you will always get how much time has elapsed from an arbitrary, fixed point in the past that is unaffected by clock skew or other changes in the wall-clock. This is useful for benchmarking how long an operation takes, as it's unaffected by the actual system time.

The general rule of thumb is that you should use ftime() with no arguments for telling time and ftime() with the monotonic clock argument for measuring the passage of time.

**Function: `ctime`**

ctime -- interprets time as a time, using the same representation as given in the description of `time()`, above, and converts it into a 28-character, human-readable string

str `ctime` ([int time])

The string will be in the following format:

```
Mon Aug 13 19:13:20 1990 PDT
```

If the current day of the month is less than 10, then an extra blank appears between the month and the day:

```
Mon Apr  1 14:10:43 1991 PST
```

If time is not provided, then the current time is used.

Note that `ctime()` interprets time for the local time zone of the computer on which the MOO server is running.

##### MOO-Code Evaluation and Task Manipulation

**Function: `raise`**

raise -- raises code as an error in the same way as other MOO expressions, statements, and functions do

none `raise` (code [, str message [, value]])

Message, which defaults to the value of `tostr(code)`, and value, which defaults to zero, are made available to any `try`-`except` statements that catch the error. If the error is not caught, then message will appear on the first line of the traceback printed to the user.

**Function: `call_function`**

call_function -- calls the built-in function named func-name, passing the given arguments, and returns whatever that function returns

value `call_function` (str func-name, arg, ...)

Raises `E_INVARG` if func-name is not recognized as the name of a known built-in function.  This allows you to compute the name of the function to call and, in particular, allows you to write a call to a built-in function that may or may not exist in the particular version of the server you're using.

**Function: `function_info`**

function_info -- returns descriptions of the built-in functions available on the server

list `function_info` ([str name])

If name is provided, only the description of the function with that name is returned. If name is omitted, a list of descriptions is returned, one for each function available on the server. Raised `E_INVARG` if name is provided but no function with that name is available on the server.

Each function description is a list of the following form:

```
{name, min-args, max-args, types
```

where name is the name of the built-in function, min-args is the minimum number of arguments that must be provided to the function, max-args is the maximum number of arguments that can be provided to the function or `-1` if there is no maximum, and types is a list of max-args integers (or min-args if max-args is `-1`), each of which represents the type of argument required in the corresponding position. Each type number is as would be returned from the `typeof()` built-in function except that `-1` indicates that any type of value is acceptable and `-2` indicates that either integers or floating-point numbers may be given. For example, here are several entries from the list:

```
{"listdelete", 2, 2, {4, 0}}
{"suspend", 0, 1, {0}}
{"server_log", 1, 2, {2, -1}}
{"max", 1, -1, {-2}}
{"tostr", 0, -1, {}}
```

`listdelete()` takes exactly 2 arguments, of which the first must be a list (`LIST == 4`) and the second must be an integer (`INT == 0`).  `suspend()` has one optional argument that, if provided, must be a number (integer or float). `server_log()` has one required argument that must be a string (`STR == 2`) and one optional argument that, if provided, may be of any type.  `max()` requires at least one argument but can take any number above that, and the first argument must be either an integer or a floating-point number; the type(s) required for any other arguments can't be determined from this description. Finally, `tostr()` takes any number of arguments at all, but it can't be determined from this description which argument types would be acceptable in which positions.

**Function: `eval`**

eval -- the MOO-code compiler processes string as if it were to be the program associated with some verb and, if no errors are found, that fictional verb is invoked

list `eval` (str string)

If the programmer is not, in fact, a programmer, then `E_PERM` is raised. The normal result of calling `eval()` is a two element list.  The first element is true if there were no compilation errors and false otherwise. The second element is either the result returned from the fictional verb (if there were no compilation errors) or a list of the compiler's error messages (otherwise).

When the fictional verb is invoked, the various built-in variables have values as shown below:

player    the same as in the calling verb
this      #-1
caller    the same as the initial value of this in the calling verb

args      {}
argstr    ""

verb      ""
dobjstr   ""
dobj      #-1
prepstr   ""
iobjstr   ""
iobj      #-1

The fictional verb runs with the permissions of the programmer and as if its `d` permissions bit were on.

```
eval("return 3 + 4;")   =>   {1, 7}
```

**Function: `set_task_perms`**

set_task_perms -- changes the permissions with which the currently-executing verb is running to be those of who

one `set_task_perms` (obj who)

If the programmer is neither who nor a wizard, then `E_PERM` is raised.
> Note: This does not change the owner of the currently-running verb, only the permissions of this particular invocation. It is used in verbs owned by wizards to make themselves run with lesser (usually non-wizard) permissions.

**Function: `caller_perms`**

caller_perms -- returns the permissions in use by the verb that called the currently-executing verb

obj `caller_perms` ()

If the currently-executing verb was not called by another verb (i.e., it is the first verb called in a command or server task), then `caller_perms()` returns `#-1`.

**Function: `set_task_local`**

set_task_local -- Sets a value that gets associated with the current running task. 

void set_task_local(ANY value)

This value persists across verb calls and gets reset when the task is killed, making it suitable for securely passing sensitive intermediate data between verbs. The value can then later be retrieved using the `task_local` function.

```
set_task_local("arbitrary data")
set_task_local({"list", "of", "arbitrary", "data"})
```

**Function: `task_local`**

task_local -- Returns the value associated with the current task. The value is set with the `set_task_local` function.

mixed `task_local` ()

**Function: `threads`**

threads -- When one or more MOO processes are suspended and working in a separate thread, this function will return a LIST of handlers to those threads. These handlers can then be passed to `thread_info' for more information.

list `threads`()

**Function: `set_thread_mode`**

int `set_thread_mode`([INT mode])

With no arguments specified, set_thread_mode will return the current thread mode for the verb. A value of 1 indicates that threading is enabled for functions that support it. A value of 0 indicates that threading is disabled and all functions will execute in the main MOO thread, as functions have done in default LambdaMOO since version 1.

If you specify an argument, you can control the thread mode of the current verb. A mode of 1 will enable threading and a mode of 0 will disable it. You can invoke this function multiple times if you want to disable threading for a single function call and enable it for the rest.

When should you disable threading? In general, threading should be disabled in verbs where it would be undesirable to suspend(). Each threaded function will immediately suspend the verb while the thread carries out its work. This can have a negative effect when you want to use these functions in verbs that cannot or should not suspend, like $sysobj:do_command or $sysobj:do_login_command.

Note that the threading mode affects the current verb only and does NOT affect verbs called from within that verb.

**Function: `thread_info`**

thread_info -- If a MOO task is running in another thread, its thread handler will give you information about that thread. 

list `thread_info`(INT thread handler)

The information returned in a LIST will be:

English Name: This is the name the programmer of the builtin function has given to the task being executed.

Active: 1 or 0 depending upon whether or not the MOO task has been killed. Not all threads cleanup immediately after the MOO task dies.

**Function: `thread_pool`**

void `thread_pool`(STR function, STR pool [, INT value])

This function allows you to control any thread pools that the server created at startup. It should be used with care, as it has the potential to create disasterous consequences if used incorrectly.

The function parameter is the function you wish to perform on the thread pool. The functions available are:

INIT: Control initialization of a thread pool.

The pool parameter controls which thread pool you wish to apply the designated function to. At the time of writing, the server creates the following thread pool:

MAIN: The main thread pool where threaded built-in function work takes place.

Finally, value is the value you want to pass to the function of pool. The following functions accept the following values:

INIT: The number of threads to spawn. NOTE: When executing this function, the existing pool will be destroyed and a new one created in its place.

Examples:

```
thread_pool("INIT", "MAIN", 1)     => Replace the existing main thread pool with a new pool consisting of a single thread.
```

**Function: `ticks_left`**

ticks_left -- return the number of ticks left to the current task before it will be forcibly terminated

int `ticks_left` () **Function: `seconds_left`**

seconds_left -- return the number of seconds left to the current task before it will be forcibly terminated

int `seconds_left` ()

These are useful, for example, in deciding when to call `suspend()` to continue a long-lived computation.

**Function: `task_id`**

task_id -- returns the non-zero, non-negative integer identifier for the currently-executing task

int `task_id` ()

Such integers are randomly selected for each task and can therefore safely be used in circumstances where unpredictability is required.

**Function: `suspend`**

suspend -- suspends the current task, and resumes it after at least seconds seconds

value `suspend` ([int|float seconds])

Sub-second suspend (IE: 0.1) is possible. If seconds is not provided, the task is suspended indefinitely; such a task can only be resumed by use of the `resume()` function.

When the task is resumed, it will have a full quota of ticks and seconds. This function is useful for programs that run for a long time or require a lot of ticks. If seconds is negative, then `E_INVARG` is raised. `Suspend()` returns zero unless it was resumed via `resume()`, in which case it returns the second argument given to that function.

In some sense, this function forks the 'rest' of the executing task. However, there is a major difference between the use of `suspend(seconds)` and the use of the `fork (seconds)`. The `fork` statement creates a new task (a _forked task_) while the currently-running task still goes on to completion, but a `suspend()` suspends the currently-running task (thus making it into a _suspended task_). This difference may be best explained by the following examples, in which one verb calls another:

```
.program   #0:caller_A
#0.prop = 1;
#0:callee_A();
#0.prop = 2;
.

.program   #0:callee_A
fork(5)
  #0.prop = 3;
endfork
.

.program   #0:caller_B
#0.prop = 1;
#0:callee_B();
#0.prop = 2;
.

.program   #0:callee_B
suspend(5);
#0.prop = 3;
.
```

Consider `#0:caller_A`, which calls `#0:callee_A`. Such a task would assign 1 to `#0.prop`, call `#0:callee_A`, fork a new task, return to `#0:caller_A`, and assign 2 to `#0.prop`, ending this task. Five seconds later, if the forked task had not been killed, then it would begin to run; it would assign 3 to `#0.prop` and then stop. So, the final value of `#0.prop` (i.e., the value after more than 5 seconds) would be 3.

Now consider `#0:caller_B`, which calls `#0:callee_B` instead of `#0:callee_A`. This task would assign 1 to `#0.prop`, call `#0:callee_B`, and suspend. Five seconds later, if the suspended task had not been killed, then it would resume; it would assign 3 to `#0.prop`, return to `#0:caller_B`, and assign 2 to `#0.prop`, ending the task. So, the final value of `#0.prop` (i.e., the value after more than 5 seconds) would be 2.

A suspended task, like a forked task, can be described by the `queued_tasks()` function and killed by the `kill_task()` function. Suspending a task does not change its task id. A task can be suspended again and again by successive calls to `suspend()`.

By default, there is no limit to the number of tasks any player may suspend, but such a limit can be imposed from within the database. See the chapter on server assumptions about the database for details.

**Function: `resume`**

resume -- immediately ends the suspension of the suspended task with the given task-id; that task's call to `suspend()` will return value, which defaults to zero

none `resume` (int task-id [, value])

If value is of type `ERR`, it will be raised, rather than returned, in the suspended task. `Resume()` raises `E_INVARG` if task-id does not specify an existing suspended task and `E_PERM` if the programmer is neither a wizard nor the owner of the specified task.

**Function: `yin`**

yin -- Suspend the current task if it's running out of ticks or seconds.

int `yin`([INT time, INT minimum ticks, INT minimum seconds] )

`yin` stands for yield if needed.

This is meant to provide similar functionality to the LambdaCore-based suspend_if_needed verb or manually specifying something like: ticks_left() < 2000 && suspend(0)

Time: How long to suspend the task. Default: 0

Minimum ticks: The minimum number of ticks the task has left before suspending.

Minimum seconds: The minimum number of seconds the task has left before suspending.

**Function: `queue_info`**

queue_info -- if player is omitted, returns a list of object numbers naming all players that currently have active task queues inside the server

list `queue_info` ([obj player])
map `queue_info` ([obj player])

If player is provided, returns the number of background tasks currently queued for that user. It is guaranteed that `queue_info(X)` will return zero for any X not in the result of `queue_info()`.

If the caller is a wizard a map of debug information about task queues will be returned.

**Function: `queued_tasks`**

queued_tasks -- returns information on each of the background tasks (i.e., forked, suspended or reading) owned by the programmer (or, if the programmer is a wizard, all queued tasks)

list `queued_tasks` ([INT show-runtime [, INT count-only])

The returned value is a list of lists, each of which encodes certain information about a particular queued task in the following format:

```
{task-id, start-time, x, y, programmer, verb-loc, verb-name, line, this, task-size}
```

where task-id is an integer identifier for this queued task, start-time is the time after which this task will begin execution (in time() format), x and y are obsolete values that are no longer interesting, programmer is the permissions with which this task will begin execution (and also the player who owns this task), verb-loc is the object on which the verb that forked this task was defined at the time, verb-name is that name of that verb, line is the number of the first line of the code in that verb that this task will execute, this is the value of the variable `this` in that verb, and task-size is the size of the task in bytes. For reading tasks, start-time is -1. 

The x and y fields are now obsolete and are retained only for backward-compatibility reasons. They may be reused for new purposes in some future version of the server.

If `show-runtime` is true, all variables present in the task are presented in a map with the variable name as the key and its value as the value.     

If `count-only` is true, then only the number of tasks is returned. This is significantly more performant than length(queued_tasks()).

> Warning: If you are upgrading to ToastStunt from a version of LambdaMOO prior to 1.8.1 you will need to dump your database, reboot into LambdaMOO emergency mode, and kill all your queued_tasks() before dumping the DB again. Otherwise, your DB will not boot into ToastStunt.

**Function: `kill_task`**

kill_task -- removes the task with the given task-id from the queue of waiting tasks

none `kill_task` (int task-id)

If the programmer is not the owner of that task and not a wizard, then `E_PERM` is raised. If there is no task on the queue with the given task-id, then `E_INVARG` is raised.

**Function: `finished_tasks()`**

finished_tasks -- returns a list of the last X tasks to finish executing, including their total execution time

list `finished_tasks`()

When enabled (via SAVE_FINISHED_TASKS in options.h), the server will keep track of the execution time of every task that passes through the interpreter. This data is then made available to the database in two ways.

The first is the finished_tasks() function. This function will return a list of maps of the last several finished tasks (configurable via $server_options.finished_tasks_limit) with the following information:

| Value      | Description                                                                           |
| ---------- | ------------------------------------------------------------------------------------- |
| foreground | 1 if the task was a foreground task, 0 if it was a background task                    |
| fullverb   | the full name of the verb, including aliases                                          |
| object     | the object that defines the verb                                                      |
| player     | the player that initiated the task                                                    |
| programmer | the programmer who owns the verb                                                      |
| receiver   | typically the same as 'this' but could be the handler in the case of primitive values |
| suspended  | whether the task was suspended or not                                                 |
| this       | the actual object the verb was called on                                              |
| time | the total time it took the verb to run), and verb (the name of the verb call or command typed |

The second is via the $handle_lagging_task verb. When the execution threshold defined in $server_options.task_lag_threshold is exceeded, the server will write an entry to the log file and call the $handle_lagging_task verb with the call stack of the task as well as the execution time.

> Note: This builtin must be enabled in options.h to be used.

**Function: `callers`**

callers -- returns information on each of the verbs and built-in functions currently waiting to resume execution in the current task

list `callers` ([include-line-numbers])

When one verb or function calls another verb or function, execution of the caller is temporarily suspended, pending the called verb or function returning a value. At any given time, there could be several such pending verbs and functions: the one that called the currently executing verb, the verb or function that called that one, and so on. The result of `callers()` is a list, each element of which gives information about one pending verb or function in the following format:

```
{this, verb-name, programmer, verb-loc, player, line-number}
```

For verbs, this is the initial value of the variable `this` in that verb, verb-name is the name used to invoke that verb, programmer is the player with whose permissions that verb is running, verb-loc is the object on which that verb is defined, player is the initial value of the variable `player` in that verb, and line-number indicates which line of the verb's code is executing. The line-number element is included only if the include-line-numbers argument was provided and true.

For functions, this, programmer, and verb-loc are all `#-1`, verb-name is the name of the function, and line-number is an index used internally to determine the current state of the built-in function. The simplest correct test for a built-in function entry is

```
(VERB-LOC == #-1  &&  PROGRAMMER == #-1  &&  VERB-name != "")
```

The first element of the list returned by `callers()` gives information on the verb that called the currently-executing verb, the second element describes the verb that called that one, and so on. The last element of the list describes the first verb called in this task.

**Function: `task_stack`**

task_stack -- returns information like that returned by the `callers()` function, but for the suspended task with the given task-id; the include-line-numbers argument has the same meaning as in `callers()`

list `task_stack` (int task-id [, INT include-line-numbers [, INT include-variables])

Raises `E_INVARG` if task-id does not specify an existing suspended task and `E_PERM` if the programmer is neither a wizard nor the owner of the specified task.

If include-line-numbers is passed and true, line numbers will be included.

If include-variables is passed and true, variables will be included with each frame of the provided task.

##### Administrative Operations

**Function: `server_version`**

server_version -- returns a string giving the version number of the running MOO server

str `server_version` ([int with-details])

If with-details is provided and true, returns a detailed list including version number as well as compilation options.

**Function `load_server_options`**
load_server_options -- This causes the server to consult the current values of properties on $server_options, updating the corresponding serveroption settings

none `load_server_options` ()

For more information see section Server Options Set in the Database.. If the programmer is not a wizard, then E_PERM is raised.

**Function: `server_log`**

server_log -- The text in message is sent to the server log with a distinctive prefix (so that it can be distinguished from server-generated messages)

none server_log (str message [, int level])

If the programmer is not a wizard, then E_PERM is raised. 

If level is provided and is an integer between 0 and 7 inclusive, then message is marked in the server log as one of eight predefined types, from simple log message to error message. Otherwise, if level is provided and true, then message is marked in the server log as an error.

**Function: `renumber`**

renumber -- the object number of the object currently numbered object is changed to be the least nonnegative object number not currently in use and the new object number is returned

obj `renumber` (obj object)

If object is not valid, then `E_INVARG` is raised. If the programmer is not a wizard, then `E_PERM` is raised. If there are no unused nonnegative object numbers less than object, then object is returned and no changes take place.

The references to object in the parent/children and location/contents hierarchies are updated to use the new object number, and any verbs, properties and/or objects owned by object are also changed to be owned by the new object number. The latter operation can be quite time consuming if the database is large. No other changes to the database are performed; in particular, no object references in property values or verb code are updated.

This operation is intended for use in making new versions of the ToastCore database from the then-current ToastStunt database, and other similar situations. Its use requires great care.

**Function: `reset_max_object`**

reset_max_object -- the server's idea of the highest object number ever used is changed to be the highest object number of a currently-existing object, thus allowing reuse of any higher numbers that refer to now-recycled objects

none `reset_max_object` ()

If the programmer is not a wizard, then `E_PERM` is raised.

This operation is intended for use in making new versions of the ToastCore database from the then-current ToastStunt database, and other similar situations. Its use requires great care.

**Function: `memory_usage`**

memory_usage -- Return statistics concerning the server's consumption of system memory.

list `memory_usage` ()

The result is a list in the following format:

{total memory used, resident set size, shared pages, text, data + stack}

**Function: `usage`**

usage -- Return statistics concerning the server the MOO is running on.

list `usage` ()

The result is a list in the following format:

```
{{load averages}, user time, system time, page reclaims, page faults, block input ops, block output ops, voluntary context switches, involuntary context switches, signals received}
```

**Function: `dump_database`**

dump_database -- requests that the server checkpoint the database at its next opportunity

none `dump_database` ()

It is not normally necessary to call this function; the server automatically checkpoints the database at regular intervals; see the chapter on server assumptions about the database for details. If the programmer is not a wizard, then `E_PERM` is raised.

**Function: `panic`**

panic -- Unceremoniously shut down the server, mimicking the behavior of a fatal error.

void panic([STR message])

The database will NOT be dumped to the file specified when starting the server. A new file will be created with the name of your database appended with .PANIC.

> Warning: Don't run this unless you really want to panic your server.

**Function: `db_disk_size`**

db_disk_size -- returns the total size, in bytes, of the most recent full representation of the database as one or more disk files

int `db_disk_size` ()

Raises `E_QUOTA` if, for some reason, no such on-disk representation is currently available.

**Function: `exec`**

exec -- Asynchronously executes the specified external executable, optionally sending input. 

list `exec` (LIST command[, STR input][, LIST environment variables])

Returns the process return code, output and error. If the programmer is not a wizard, then E_PERM is raised.

The first argument must be a list of strings, or E_INVARG is raised. The first string is the path to the executable and is required. The rest are command line arguments passed to the executable.

The path to the executable may not start with a slash (/) or dot-dot (..), and it may not contain slash-dot (/.) or dot-slash (./), or E_INVARG is raised. If the specified executable does not exist or is not a regular file, E_INVARG is raised.

If the string input is present, it is written to standard input of the executing process.

Additionally, you can provide a list of environment variables to set in the shell.

When the process exits, it returns a list of the form:

```
{code, output, error}
```

code is the integer process exit status or return code. output and error are strings of data that were written to the standard output and error of the process.

The specified command is executed asynchronously. The function suspends the current task and allows other tasks to run until the command finishes. Tasks suspended this way can be killed with kill_task().

The strings, input, output and error are all MOO binary strings.

All external executables must reside in the executables directory.

```
exec({"cat", "-?"})                                      {1, "", "cat: illegal option -- ?~0Ausage: cat [-benstuv] [file ...]~0A"}
exec({"cat"}, "foo")                                     {0, "foo", ""}
exec({"echo", "one", "two"})                             {0, "one two~0A", ""}
```

**Function: `shutdown`**

shutdown -- requests that the server shut itself down at its next opportunity

none `shutdown` ([str message])

Before doing so, a notice (incorporating message, if provided) is printed to all connected players. If the programmer is not a wizard, then `E_PERM` is raised.

**Function: `verb_cache_stats`**

**Function: `log_cache_stats`**

list verb_cache_stats ()

none log_cache_stats ()

The server caches verbname-to-program lookups to improve performance. These functions respectively return or write to the server log file the current cache statistics. For verb_cache_stats the return value will be a list of the form

```
{hits, negative_hits, misses, table_clears, histogram},
```

though this may change in future server releases. The cache is invalidated by any builtin function call that may have an effect on verb lookups (e.g., delete_verb()). 


### Server Commands and Database Assumptions

This chapter describes all of the commands that are built into the server and every property and verb in the database specifically accessed by the server.  Aside from what is listed here, no assumptions are made by the server concerning the contents of the database.

#### Command Lines That Receive Special Treatment

As was mentioned in the chapter on command parsing, there are a number of commands and special prefixes whose interpretation is fixed by the server. Examples include the flush command and the five intrinsic commands (PREFIX, OUTPUTPREFIX, SUFFIX, OUTPUTSUFFIX, and .program).

This section discusses all of these built-in pieces of the command-interpretation process in the order in which they occur.

##### Flushing Unprocessed Input

It sometimes happens that a user changes their mind about having typed one or more lines of input and would like to `untype` them before the server actually gets around to processing them. If they react quickly enough, they can type their connection`s defined flush command; when the server first reads that command from the network, it immediately and completely flushes any as-yet unprocessed input from that user, printing a message to the user describing just which lines of input were discarded, if any.

> Fine point: The flush command is handled very early in the server`s processing of a line of input, before the line is entered into the task queue for the connection and well before it is parsed into words like other commands. For this reason, it must be typed exactly as it was defined, alone on the line, without quotation marks, and without any spaces before or after it.

When a connection is first accepted by the server, it is given an initial flush command setting taken from the current default. This initial setting can be changed later using the set_connection_option() command.

By default, each connection is initially given `.flush` as its flush command. If the property $server_options.default_flush_command exists, then its value overrides this default. If $server_options.default_flush_command is a non-empty string, then that string is the flush command for all new connections; otherwise, new connections are initially given no flush command at all.

##### Out-of-Band Processing

It is possible to compile the server to recognize an out-of-band prefix and an out-of-band quoting prefix for input lines. These are strings that the server will check for at the beginning of every unflushed line of input from a non-binary connection, regardless of whether or not a player is logged in and regardless of whether or not reading tasks are waiting for input on that connection.

This check can be disabled entirely by setting connection option "disable-oob", in which case none of the rest of this section applies, i.e., all subsequent unflushed lines on that connection will be available unchanged for reading tasks or normal command parsing. 

##### Quoted Lines

We first describe how to ensure that a given input line will not be processed as an out-of-band command.

If a given line of input begins with the defined out-of-band quoting prefix (`#$"` by default), that prefix is stripped. The resulting line is then available to reading tasks or normal command parsing in the usual way, even if said resulting line now happens to begin with either the out-of-band prefix or the out-of-band quoting prefix.

For example, if a player types
 	
```
#$"#$#mcp-client-set-type fancy
```

the server would behave exactly as if connection option "disable-oob" were set true and the player had instead typed
 	
```
#$#mcp-client-set-type fancy
```

##### Commands

If a given line of input begins with the defined out-of-band prefix (`#$#` by default), then it is not treated as a normal command or given as input to any reading task. Instead, the line is parsed into a list of words in the usual way and those words are given as the arguments in a call to $do_out_of_band_command().

If this verb does not exist or is not executable, the line in question will be completely ignored.

For example, with the default out-of-band prefix, the line of input
 	
```
#$#mcp-client-set-type fancy
```

would result in the following call being made in a new server task:
 	
```
$do_out_of_band_command("#$#mcp-client-set-type", "fancy")
```

During the call to $do_out_of_band_command(), the variable player is set to the object number representing the player associated with the connection from which the input line came. Of course, if that connection has not yet logged in, the object number will be negative. Also, the variable argstr will have as its value the unparsed input line as received on the network connection.

Out-of-band commands are intended for use by advanced client programs that may generate asynchronous events of which the server must be notified. Since the client cannot, in general, know the state of the player`s connection (logged-in or not, reading task or not), out-of-band commands provide the only reliable client-to-server communications channel. 

[Telnet IAC](http://www.faqs.org/rfcs/rfc854.html) commands will also get captured and passed, as binary strings, to a `do_out_of_band_command` verb on the listener.

##### Command-Output Delimiters

> Warning: This is a deprecated feature

Every MOO network connection has associated with it two strings, the `output prefix` and the `output suffix`. Just before executing a command typed on that connection, the server prints the output prefix, if any, to the player. Similarly, just after finishing the command, the output suffix, if any, is printed to the player. Initially, these strings are not defined, so no extra printing takes place.

The `PREFIX` and `SUFFIX` commands are used to set and clear these strings. They have the following simple syntax:

```
PREFIX  output-prefix
SUFFIX  output-suffix
```

That is, all text after the command name and any following spaces is used as the new value of the appropriate string. If there is no non-blank text after the command string, then the corresponding string is cleared. For compatibility with some general MUD client programs, the server also recognizes `OUTPUTPREFIX` as a synonym for `PREFIX` and `OUTPUTSUFFIX` as a synonym for `SUFFIX`.

These commands are intended for use by programs connected to the MOO, so that they can issue MOO commands and reliably determine the beginning and end of the resulting output. For example, one editor-based client program sends this sequence of commands on occasion:

```
PREFIX >>MOO-Prefix<<
SUFFIX >>MOO-Suffix<<
@list object:verb without numbers
PREFIX
SUFFIX
```

The effect of which, in a ToastCore-derived database, is to print out the code for the named verb preceded by a line containing only `>>MOO-Prefix<<` and followed by a line containing only `>>MOO-Suffix<<`. This enables the editor to reliably extract the program text from the MOO output and show it to the user in a separate editor window. There are many other possible uses.

> Warning: If the command thus bracketed calls suspend(), its output will be deemed “finished” then and there; the suffix thus appears at that point and not, as one might expect, later when the resulting background task has finally returned from its top-level verb call. Thus, use of this feature (which was designed before suspend() existed) is no longer recommended. 

The built-in function `output_delimiters()` can be used by MOO code to find out the output prefix and suffix currently in effect on a particular network connection.

#### The .program Command

The `.program` command is a common way for programmers to associate a particular MOO-code program with a particular verb. It has the following syntax:

```
.program object:verb
...several lines of MOO code...
.
```

That is, after typing the `.program` command, then all lines of input from the player are considered to be a part of the MOO program being defined. This ends as soon as the player types a line containing only a dot (`.`). When that line is received, the accumulated MOO program is checked for proper MOO syntax and, if correct, associated with the named verb.

If, at the time the line containing only a dot is processed, (a) the player is not a programmer, (b) the player does not have write permission on the named verb, or (c) the property `$server_options.protect_set_verb_code` exists and has a true value and the player is not a wizard, then an error message is printed and the named verb's program is not changed.

In the `.program` command, object may have one of three forms:

* The name of some object visible to the player. This is exactly like the kind of matching done by the server for the direct and indirect objects of ordinary commands. See the chapter on command parsing for details. Note that the special names `me` and `here` may be used.
* An object number, in the form `#number`.
* A _system property_ (that is, a property on `#0`), in the form `$name`. In this case, the current value of `#0.name` must be a valid object.

#### Initial Punctuation in Commands

The server interprets command lines that begin with any of the following characters specially:

```
"        :        ;
```

Before processing the command, the initial punctuation character is replaced by the corresponding word below, followed by a space:

```
say      emote    eval
```

For example, the command line

```
"Hello, there.
```

is transformed into

```
say Hello, there.
```

before parsing.

### Server Assumptions About the Database

There are a small number of circumstances under which the server directly and specifically accesses a particular verb or property in the database. This section gives a complete list of such circumstances.

#### Server Options Set in the Database

Many optional behaviors of the server can be controlled from within the database by creating the property `#0.server_options` (also known as `$server_options`), assigning as its value a valid object number, and then defining various properties on that object. At a number of times, the server checks for whether the property `$server_options` exists and has an object number as its value. If so, then the server looks for a variety of other properties on that `$server_options` object and, if they exist, uses their values to control how the server operates.

The specific properties searched for are each described in the appropriate section below, but here is a brief list of all of the relevant properties for ease of reference:

| Property                         | Description                                                                                                                                                |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| bg_seconds                       | The number of seconds allotted to background tasks.                                                                                                        |
| bg_ticks                         | The number of ticks allotted to background tasks.                                                                                                          |
| connect_timeout                  | The maximum number of seconds to allow an un-logged-in in-bound connection to remain open.                                                                 |
| default_flush_command            | The initial setting of each new connection&apos;s flush command.                                                                                           |
| fg_seconds                       | The number of seconds allotted to foreground tasks.                                                                                                        |
| fg_ticks                         | The number of ticks allotted to foreground tasks.                                                                                                          |
| max_stack_depth                  | The maximum number of levels of nested verb calls. Only used if it is higher than default                                                                  |
| name_lookup_timeout              | The maximum number of seconds to wait for a network hostname/address lookup.                                                                               |
| outbound_connect_timeout         | The maximum number of seconds to wait for an outbound network connection to successfully open.                                                             |
| protect_`property`               | Restrict reading/writing of built-in `property` to wizards.                                                                                                |
| protect_`function`               | Restrict use of built-in `function` to wizards.                                                                                                            |
| queued_task_limit                | The maximum number of forked or suspended tasks any player can have queued at a given time.                                                                |
| support_numeric_verbname_strings | Enables use of an obsolete verb-naming mechanism.                                                                                                          |
| max_queued_output                | The maximum number of output characters the server is willing to buffer for any given network connection before discarding old output to make way for new. |
| dump_interval                    | an int in seconds for how often to checkpoint the database.                                                                                                |
| proxy_rewrite                    | control whether IPs from proxies get rewritten.                                                                                                            |
| file_io_max_files                | allow DB-changeable limits on how many files can be opened at once.                                                                                        |
| sqlite_max_handles               | allow DB-changeable limits on how many SQLite connections can be opened at once.                                                                           |
| task_lag_threshold               | override default task_lag_threshold for handling lagging tasks                                                                                             |
| finished_tasks_limit             | override default finished_tasks_limit (enables the finished_tasks function and define how many tasks get saved by default)                                 |
| no_name_lookup                   | override default no_name_lookup (disables automatic DNS name resolution on new connections)                                                                |
| max_list_concat                  | limit the size of user-constructed lists                                                                                                                   |
| max_string_concat                | limit the size of user-constructed strings                                                                                                                 |
| max_concat_catchable | govern whether violating concat size limits causes out-of-seconds or E_QUOTA error |

> Note: If you override a default value that was defined in options.h (such as no_name_lookup or finished_tasks_limit, or many others) you will need to call `load_server_options()` for your changes to take affect.

> Note: Verbs defined on #0 are not longer subject to the wiz-only permissions check on built-in functions generated by defining $server_options.protect_FOO with a true value.  Thus, you can now write a `wrapper' for a built-in function without having to re-implement all of the server's built-in permissions checks for that function.  

> Note: If a built-in function FOO has been made wiz-only (by defining $server_options.protect_FOO with a true value) and a call is made to that function from a non-wiz verb not defined on #0 (that is, if the server is about to raise E_PERM), the server first checks to see if the verb #0:bf_FOO exists.  If so, it calls it instead of raising E_PERM and returns or raises whatever it returns or raises.

> Note: options.h #defines IGNORE_PROP_PROTECTED by default. If it is defined, the server ignores all attempts to protect built-in properties (such as $server_options.protect_location). Protecting properties is a significant performance hit, and most MOOs do not use this functionality.

#### Server Messages Set in the Database

There are a number of circumstances under which the server itself generates messages on network connections. Most of these can be customized or even eliminated from within the database. In each such case, a property on `$server_options` is checked at the time the message would be printed. If the property does not exist, a default message is printed. If the property exists and its value is not a string or a list containing strings, then no message is printed at all. Otherwise, the string(s) are printed in place of the default message, one string per line. None of these messages are ever printed on an outbound network connection created by the function `open_network_connection()`.

The following list covers all of the customizable messages, showing for each the name of the relevant property on `$server_options`, the default message, and the circumstances under which the message is printed:

| Default Message                                                                                                                   | Description                                                                                                                                           |
| --------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| boot_msg = &quot;*** Disconnected ***&quot;                                                                                       | The function boot_player() was called on this connection.                                                                                             |
| connect_msg = &quot;*** Connected ***&quot;                                                                                       | The user object that just logged in on this connection existed before $do_login_command() was called.                                                 |
| create_msg = &quot;*** Created ***&quot;                                                                                          | The user object that just logged in on this connection did not exist before $do_login_command() was called.                                           |
| recycle_msg = &quot;*** Recycled ***&quot;                                                                                        | The logged-in user of this connection has been recycled or renumbered (via the renumber() function).                                                  |
| redirect_from_msg = &quot;*** Redirecting connection to new port ***&quot;                                                        | The logged-in user of this connection has just logged in on some other connection.                                                                    |
| redirect_to_msg = &quot;*** Redirecting old connection to this port ***&quot;                                                     | The user who just logged in on this connection was already logged in on some other connection.                                                        |
| server_full_msg Default:  *** Sorry, but the server cannot accept any more connections right now.<br> *** Please try again later. | This connection arrived when the server really couldn&apos;t accept any more connections, due to running out of a critical operating system resource. |
| timeout_msg = &quot;*** Timed-out waiting for login. ***&quot; | This in-bound network connection was idle and un-logged-in for at least CONNECT_TIMEOUT seconds (as defined in the file options.h when the server was compiled). |

> Fine point: If the network connection in question was received at a listening point (established by the `listen()` function) handled by an object obj other than `#0`, then system messages for that connection are looked for on `obj.server_options`; if that property does not exist, then `$server_options` is used instead.

#### Checkpointing the Database

The server maintains the entire MOO database in main memory, not on disk. It is therefore necessary for it to dump the database to disk if it is to persist beyond the lifetime of any particular server execution. The server is careful to dump the database just before shutting down, of course, but it is also prudent for it to do so at regular intervals, just in case something untoward happens.

//TODO: is the date here still true in 64bit time?

To determine how often to make these _checkpoints_ of the database, the server consults the value of `$server_options.dump_interval`. If it exists and its value is an integer greater than or equal to 60, then it is taken as the number of seconds to wait between checkpoints; otherwise, the server makes a new checkpoint every 3600 seconds (one hour). If the value of `$server_options.dump_interval` implies that the next checkpoint should be scheduled at a time after 3:14:07 a.m. on Tuesday, January 19, 2038, then the server instead uses the default value of 3600 seconds in the future.

The decision about how long to wait between checkpoints is made again immediately after each one begins. Thus, changes to `$server_options.dump_interval` will take effect after the next checkpoint happens.

Whenever the server begins to make a checkpoint, it makes the following verb call:

```
$checkpoint_started()
```

When the checkpointing process is complete, the server makes the following verb call:

```
$checkpoint_finished(success)
```

where success is true if and only if the checkpoint was successfully written on the disk. Checkpointing can fail for a number of reasons, usually due to exhaustion of various operating system resources such as virtual memory or disk space. It is not an error if either of these verbs does not exist; the corresponding call is simply skipped.

### Networking

#### Accepting and Initiating Network Connections

When the server first accepts a new, incoming network connection, it is given the low-level network address of computer on the other end. It immediately attempts to convert this address into the human-readable host name that will be entered in the server log and returned by the `connection_name()` function. This conversion can, for the TCP/IP networking configurations, involve a certain amount of communication with remote name servers, which can take quite a long time and/or fail entirely. While the server is doing this conversion, it is not doing anything else at all; in particular, it it not responding to user commands or executing MOO tasks.

By default, the server will wait no more than 5 seconds for such a name lookup to succeed; after that, it behaves as if the conversion had failed, using instead a printable representation of the low-level address. If the property `name_lookup_timeout` exists on `$server_options` and has an integer as its value, that integer is used instead as the timeout interval.

When the `open_network_connection()` function is used, the server must again do a conversion, this time from the host name given as an argument into the low-level address necessary for actually opening the connection. This conversion is subject to the same timeout as in the in-bound case; if the conversion does not succeed before the timeout expires, the connection attempt is aborted and `open_network_connection()` raises `E_QUOTA`.

After a successful conversion, though, the server must still wait for the actual connection to be accepted by the remote computer. As before, this can take a long time during which the server is again doing nothing else. Also as before, the server will by default wait no more than 5 seconds for the connection attempt to succeed; if the timeout expires, `open_network_connection()` again raises `E_QUOTA`. This default timeout interval can also be overridden from within the database, by defining the property `outbound_connect_timeout` on `$server_options` with an integer as its value.

#### Associating Network Connections with Players

When a network connection is first made to the MOO, it is identified by a unique, negative object number. Such a connection is said to be _un-logged-in_ and is not yet associated with any MOO player object.

Each line of input on an un-logged-in connection is first parsed into words in the usual way (see the chapter on command parsing for details) and then these words are passed as the arguments in a call to the verb `$do_login_command()`. For example, the input line

```
connect Munchkin frebblebit
```

would result in the following call being made:

```
$do_login_command("connect", "Munchkin", "frebblebit")
```

In that call, the variable `player` will have as its value the negative object number associated with the appropriate network connection. The functions `notify()` and `boot_player()` can be used with such object numbers to send output to and disconnect un-logged-in connections. Also, the variable `argstr` will have as its value the unparsed command line as received on the network connection.

If `$do_login_command()` returns a valid player object and the connection is still open, then the connection is considered to have _logged into_ that player. The server then makes one of the following verbs calls, depending on the player object that was returned:

```
$user_created(player)
$user_connected(player)
$user_reconnected(player)
```

The first of these is used if the returned object number is greater than the value returned by the `max_object()` function before `$do_login_command()` was invoked, that is, it is called if the returned object appears to have been freshly created. If this is not the case, then one of the other two verb calls is used. The `$user_connected()` call is used if there was no existing active connection for the returned player object. Otherwise, the `$user_reconnected()` call is used instead.

> Fine point: If a user reconnects and the user's old and new connections are on two different listening points being handled by different objects (see the description of the `listen()` function for more details), then `user_client_disconnected` is called for the old connection and `user_connected` for the new one.

> Note: If any code suspends in do_login_command() or a verb called by do_login_command() (read(), suspend(), or any threaded function), you can no longer connect an object by returning it. This is a weird ancient MOO holdover. The best way to log a player in after suspending is to use the `switch_player()` function to switch their unlogged in negative object to their player object.

If an in-bound network connection does not successfully log in within a certain period of time, the server will automatically shut down the connection, thereby freeing up the resources associated with maintaining it. Let L be the object handling the listening point on which the connection was received (or `#0` if the connection came in on the initial listening point). To discover the timeout period, the server checks on `L.server_options` or, if it doesn't exist, on `$server_options` for a `connect_timeout` property. If one is found and its value is a positive integer, then that's the number of seconds the server will use for the timeout period. If the `connect_timeout` property exists but its value isn't a positive integer, then there is no timeout at all. If the property doesn't exist, then the default timeout is 300 seconds.

When any network connection (even an un-logged-in or outbound one) is terminated, by either the server or the client, then one of the following two verb calls is made:

```
$user_disconnected(player)
$user_client_disconnected(player)
```

The first is used if the disconnection is due to actions taken by the server (e.g., a use of the `boot_player()` function or the un-logged-in timeout described above) and the second if the disconnection was initiated by the client side.

It is not an error if any of these five verbs do not exist; the corresponding call is simply skipped.

> Note: Only one network connection can be controlling a given player object at a given time; should a second connection attempt to log in as that player, the first connection is unceremoniously closed (and `$user_reconnected()` called, as described above). This makes it easy to recover from various kinds of network problems that leave connections open but inaccessible.

When the network connection is first established, the null command is automatically entered by the server, resulting in an initial call to `$do_login_command()` with no arguments. This signal can be used by the verb to print out a welcome message, for example.

> Warning: If there is no `$do_login_command()` verb defined, then lines of input from un-logged-in connections are simply discarded. Thus, it is _necessary_ for any database to include a suitable definition for this verb.

> Note that a database with a missing or broken $do_login_command may still be accessed (and perhaps repaired) by running the server with the -e command line option. See section Emergency Wizard Mode. 

It is possible to compile the server with an option defining an `out-of-band prefix` for commands. This is a string that the server will check for at the beginning of every line of input from players, regardless of whether or not those players are logged in and regardless of whether or not reading tasks are waiting for input from those players. If a given line of input begins with the defined out-of-band prefix (leading spaces, if any, are _not_ stripped before testing), then it is not treated as a normal command or as input to any reading task. Instead, the line is parsed into a list of words in the usual way and those words are given as the arguments in a call to `$do_out_of_band_command()`. For example, if the out-of-band prefix were defined to be `#$#`, then the line of input

```
#$# client-type fancy
```

would result in the following call being made in a new server task:

```
$do_out_of_band_command("#$#", "client-type", "fancy")
```

During the call to `$do_out_of_band_command()`, the variable `player` is set to the object number representing the player associated with the connection from which the input line came. Of course, if that connection has not yet logged in, the object number will be negative. Also, the variable `argstr` will have as its value the unparsed input line as received on the network connection.

Out-of-band commands are intended for use by fancy client programs that may generate asynchronous _events_ of which the server must be notified. Since the client cannot, in general, know the state of the player's connection (logged-in or not, reading task or not), out-of-band commands provide the only reliable client-to-server communications channel.

#### Player Input Handlers

**$do_out_of_band_command**

On any connection for which the connection-option disable-oob has not been set, any unflushed incoming lines that begin with the out-of-band prefix will be treated as out-of-band commands, meaning that if the verb $do_out_of_band_command() exists and is executable, it will be called for each such line. For more on this, see Out-of-band Processing.

**$do_command**

As we previously described in The Built-in Command Parser, on any logged-in connection that

* is not the subject of a read() call,
* does not have a .program command in progress, and
* has not had its connection option hold-input set, 

any incoming line that

* has not been flushed
* is in-band (i.e., has not been consumed by out-of-band processing) and
* is not itself .program or one of the other four intrinsic commands 

will result in a call to $do_command() provided that verb exists and is executable. If this verb suspends or returns a true value, then processing of that line ends at this point, otherwise, whether the verb returned false or did not exist in the first place, the remainder of the builtin parsing process is invoked. 

### The First Tasks Run By the Server

Whenever the server is booted, there are a few tasks it runs right at the beginning, before accepting connections or getting the value of $server_options.dump_interval to schedule the first checkpoint (see below for more information on checkpoint scheduling).

First, the server calls $do_start_script() and passes in script content via the args built-in variable. The script content is specified on the command line when the server is started. The server can call this verb multiple times, once each for the -c and -f command line arguments.

Next, the server calls $user_disconnected() once for each user who was connected at the time the database file was written; this allows for any cleaning up that`s usually done when users disconnect (e.g., moving their player objects back to some `home` location, etc.).

Next, it checks for the existence of the verb $server_started(). If there is such a verb, then the server runs a task invoking that verb with no arguments and with player equal to #-1. This is useful for carefully scheduling checkpoints and for re-initializing any state that is not properly represented in the database file (e.g., re-opening certain outbound network connections, clearing out certain tables, etc.). 

### Controlling the Execution of Tasks

As described earlier, in the section describing MOO tasks, the server places limits on the number of seconds for which any task may run continuously and the number of “ticks,” or low-level operations, any task may execute in one unbroken period. By default, foreground tasks may use 60,000 ticks and five seconds, and background tasks may use 30,000 ticks and three seconds. These defaults can be overridden from within the database by defining any or all of the following properties on $server_options and giving them integer values: 

| Property   | Description                                         |
| ---------- | --------------------------------------------------- |
| bg_seconds | The number of seconds allotted to background tasks. |
| bg_ticks   | The number of ticks allotted to background tasks.   |
| fg_seconds | The number of seconds allotted to foreground tasks. |
| fg_ticks | The number of ticks allotted to foreground tasks. |

The server ignores the values of `fg_ticks` and `bg_ticks` if they are less than 100 and similarly ignores `fg_seconds` and `bg_seconds` if their values are less than 1. This may help prevent utter disaster should you accidentally give them uselessly-small values.

Recall that command tasks and server tasks are deemed _foreground_ tasks, while forked, suspended, and reading tasks are defined as _background_ tasks. The settings of these variables take effect only at the beginning of execution or upon resumption of execution after suspending or reading.

The server also places a limit on the number of levels of nested verb calls, raising `E_MAXREC` from a verb-call expression if the limit is exceeded. The limit is 50 levels by default, but this can be increased from within the database by defining the `max_stack_depth` property on `$server_options` and giving it an integer value greater than 50. The maximum stack depth for any task is set at the time that task is created and cannot be changed thereafter. This implies that suspended tasks, even after being saved in and restored from the DB, are not affected by later changes to $server_options.max_stack_depth.

Finally, the server can place a limit on the number of forked or suspended tasks any player can have queued at a given time. Each time a `fork` statement or a call to `suspend()` is executed in some verb, the server checks for a property named `queued_task_limit` on the programmer. If that property exists and its value is a non-negative integer, then that integer is the limit. Otherwise, if `$server_options.queued_task_limit` exists and its value is a non-negative integer, then that's the limit. Otherwise, there is no limit. If the programmer already has a number of queued tasks that is greater than or equal to the limit, `E_QUOTA` is raised instead of either forking or suspending. Reading tasks are affected by the queued-task limit.

### Controlling the Handling of Aborted Tasks

The server will abort the execution of tasks for either of two reasons:

1. an error was raised within the task but not caught

In each case, after aborting the task, the server attempts to call a particular _handler verb_ within the database to allow code there to handle this mishap in some appropriate way. If this verb call suspends or returns a true value, then it is considered to have handled the situation completely and no further processing will be done by the server. On the other hand, if the handler verb does not exist, or if the call either returns a false value without suspending or itself is aborted, the server takes matters into its own hands.

First, an error message and a MOO verb-call stack _traceback_ are printed to the player who typed the command that created the original aborted task, explaining why the task was aborted and where in the task the problem occurred. Then, if the call to the handler verb was itself aborted, a second error message and traceback are printed, describing that problem as well. Note that if the handler-verb call itself is aborted, no further 'nested' handler calls are made; this policy prevents what might otherwise be quite a vicious little cycle.

The specific handler verb, and the set of arguments it is passed, differs for the two causes of aborted tasks.

If an error is raised and not caught, then the verb-call

```
$handle_uncaught_error(code, msg, value, traceback, formatted)
```

is made, where code, msg, value, and traceback are the values that would have been passed to a handler in a `try`-`except` statement and formatted is a list of strings being the lines of error and traceback output that will be printed to the player if `$handle_uncaught_error` returns false without suspending.

If a task runs out of ticks or seconds, then the verb-call

```
$handle_task_timeout(resource, traceback, formatted)
```

is made, where `resource` is the appropriate one of the strings `"ticks"` or `"seconds"`, and `traceback` and `formatted` are as above.

### Matching in Command Parsing

In the process of matching the direct and indirect object strings in a command to actual objects, the server uses the value of the `aliases` property, if any, on each object in the contents of the player and the player's location.  For complete details, see the chapter on command parsing.

### Restricting Access to Built-in Properties and Functions

**Protected Properties**

A built-in property prop is deemed protected if $server_options.protect_prop exists and has a true value. However, no such property protections are recognized if the compilation option IGNORE_PROP_PROTECTED (see section Server Compilation Options) was set when building the server. 

> Note: In previous versions of the server enabling this has significant performance costs, but that has been resolved with caching lookups, and thus this option is enabled by default in ToastStunt. 

Whenever verb code attempts to read (on any object) the value of a built-in property that is protected in this way, the server raises E_PERM if the programmer is not a wizard.

**Protected Built-in Functions**

A built-in function func() is deemed protected if $server_options.protect_func exists and has a true value. If, for a given protected built-in function, a corresponding verb $bf_func() exists and its `x` bit is set, then that built-in function is also considered overridden, meaning that any call to func() from any object other than #0 will be treated as a call to $bf_func() with the same arguments, returning or raising whatever that verb returns or raises.

A call to a protected built-in function that is not overridden proceeds normally as long as either the caller is #0 or has wizard permissions; otherwise the server raises E_PERM.

Note that you must call load_server_options() in order to ensure that changes made in $server_options take effect.

### Creating and Recycling Objects

Whenever the `create()` function is used to create a new object, that object's `initialize` verb, if any, is called with no arguments. The call is simply skipped if no such verb is defined on the object.

Symmetrically, just before the `recycle()` function actually destroys an object, the object's `recycle` verb, if any, is called with no arguments.  Again, the call is simply skipped if no such verb is defined on the object.

Both `create()` and `recycle()` check for the existence of an `ownership_quota` property on the owner of the newly-created or -destroyed object. If such a property exists and its value is an integer, then it is treated as a _quota_ on object ownership. Otherwise, the following two paragraphs do not apply.

The `create()` function checks whether or not the quota is positive; if so, it is reduced by one and stored back into the `ownership_quota` property on the owner. If the quota is zero or negative, the quota is considered to be exhausted and `create()` raises `E_QUOTA`.

The `recycle()` function increases the quota by one and stores it back into the `ownership_quota` property on the owner.

### Object Movement

During evaluation of a call to the `move()` function, the server can make calls on the `accept` and `enterfunc` verbs defined on the destination of the move and on the `exitfunc` verb defined on the source.  The rules and circumstances are somewhat complicated and are given in detail in the description of the `move()` function.

### Temporarily Enabling Obsolete Server Features

If the property `$server_options.support_numeric_verbname_strings` exists and has a true value, then the server supports a obsolete mechanism for less ambiguously referring to specific verbs in various built-in functions. For more details, see the discussion given just following the description of the `verbs()` function.

### Signals to the Server

The server is able to intercept [signals](https://en.wikipedia.org/wiki/Signal_(IPC)) from the operating system and perform certain actions, a list of which can be found below. Two signals, USR1 and USR2, can be processed from within the MOO database. When SIGUSR1 or SIGUSR2 is received, the server will call `#0:handle_signal()` with the name of the signal as the only argument. If this verb returns a true value, it is assumed that the database handled it and no further action is taken. If the verb returns a negative value, the server will proceed to execute the default action for that signal. The following is a list of signals and their default actions:

| Signal | Action                        |
| ------ | ----------------------------- |
| HUP    | Panic the server.             |
| ILL    | Panic the server.             |
| QUIT   | Panic the server.             |
| SEGV   | Panic the server.             |
| BUS    | Panic the server.             |
| INT    | Cleanly shut down the server. |
| TERM   | Cleanly shut down the server. |
| USR1   | Reopen the log file.          |
| USR2   | Schedule a checkpoint to happen as soon as possible. |

For example, imagine you're a system administrator logged into the machine running the MOO. You want to shut down the MOO server, but you'd like to give players the opportunity to say goodbye to one another rather than immediately shutting the server down. You can do so by intercepting a signal in the database and invoking the @shutdown command.

```
@prog #0:handle_signal
set_task_perms(caller_perms());
{signal} = args;
if (signal == "SIGUSR2" && !$code_utils:task_valid($wiz_utils.shutdown_task))
  force_input(#2, "@shutdown in 1 Shutdown signal received.");
  force_input(#2, "yes");
  return 1;
endif
.
```

Now you can signal the MOO with the kill command: `kill -USR2 <MOO process ID`

### Server Configuration

This section discusses the options for compiling and running the server that can affect the database and how the code within it runs.

#### Server Compilation Options

The following option values are specified (via #define) in the file `options.h` in the server sources. Except for those cases where property values on $server_options take precedence, these settings cannot be changed at runtime.

This list is not intended to be exhaustive.
Network Options

| Option                   | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| NETWORK_PROTOCOL         | This specifies the underlying protocol for the server to use for all connections and will be one of the following:<br>`NP_TCP` The server uses TCP/IP protocols. <br> `NP_LOCAL` The server uses local interprocess communication mechanisms (currently either BSD UNIX-domain sockets or SYSV named pipes).<br>`NP_SINGLE` The server accepts only a single `connection` via the standard input and output streams of the server itself. Attempts to have multiple simultaneous listening points (via listen() will likewise fail. |
| DEFAULT_PORT             | (for NP_TCP) the TCP port number on which the server listens when no port-number argument is given on the command line.                                                                                                                                                                                                                                                                                                                                                                                                             |
| DEFAULT_CONNECT_FILE     | (for NP_LOCAL) the local filename through which the server will listen for connections when no connect-file-name is given on the command line.                                                                                                                                                                                                                                                                                                                                                                                      |
| OUTBOUND_NETWORK         | The server will include support for open_network_connection() if this constant is defined. If given a zero value, the function will be disabled by default and `-o` will need to be specified on the command line in order to enable it, otherwise (nonzero or blank value) the function is enabled by default and `-O` will needed to disable it. When disabled or not supported, open_network_connection() raises E_PERM whenever it is called. The NETWORK_PROTOCOL must be NP_TCP.                                              |
| MAX_QUEUED_OUTPUT        | The maximum number of output characters the server is willing to buffer for any given network connection before discarding old output to make way for new. This can be overridden in-database by adding the property `$server_options.max_queued_output` and calling `load_server_options()`.                                                                                                                                                                                                                                       |
| MAX_QUEUED_INPUT         | The maximum number of input characters the server is willing to buffer from any given network connection before it stops reading from the connection at all.                                                                                                                                                                                                                                                                                                                                                                        |
| IGNORE_PROP_PROTECTED    | Disables protection of builtin properties via $server_options.protect_property when set. See section Protected Properties.                                                                                                                                                                                                                                                                                                                                                                                                          |
| OUT_OF_BAND_PREFIX       | Specifies the out-of-band prefix. If this is defined as a non-empty string, then any lines of input from any player that begin with that prefix will not be consumed by reading tasks and will not undergo normal command parsing. See section Out-of-band Processing.                                                                                                                                                                                                                                                              |
| OUT_OF_BAND_QUOTE_PREFIX | Specifies the out-of-band quoting prefix. If this is defined as a non-empty string, then any lines of input from any player that begin with that prefix will have that prefixed stripped and the resulting string will bypass Out-of-Band Processing.                                                                                                                                                                                                                                                                               |
| DEFAULT_MAX_STACK_DEPTH  | Default value for $server_options.max_stack_depth.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| DEFAULT_FG_TICKS         | The number of ticks allotted to foreground tasks. Default value for $server_options.fg_ticks.                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| DEFAULT_BG_TICKS         | The number of ticks allotted to background tasks. Default value for $server_options.bg_ticks.                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| DEFAULT_FG_SECONDS       | The number of seconds allotted to foreground tasks. Default value for $server_options.fg_seconds.                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| DEFAULT_BG_SECONDS       | The number of seconds allotted to background tasks. Default value for $server_options.bg_seconds.                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| DEFAULT_CONNECT_TIMEOUT  | Default value for $server_options.connect_timeout.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| LOG_CODE_CHANGES         | Write to the log file who changed what verb code.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| USE_ANCESTOR_CACHE       | Determine if the server should cache the ancestors of objects to improve performance of property lookups.                                                                                                                                                                                                                                                                                                                                                                                                                           |
| OWNERSHIP_QUOTA          | Control whether or not the server's default ownership quota management is enabled or not. It defaults to disabled to allow the database to handle quota on its own.                                                                                                                                                                                                                                                                                                                                                                 |
| UNSAFE_FIO               | This allows you to skip the character by character line verification for a small performance boost. Make sure to read the disclaimer above it in options.h.                                                                                                                                                                                                                                                                                                                                                                         |
| LOG_EVALS                | Allow all evals to be written to the server log.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| MEMO_STRLEN              | Improve performance of string comparisons by using the pre-computed length of strings to rule out equality before doing a character by character comparison.                                                                                                                                                                                                                                                                                                                                                                        |
| NO_NAME_LOOKUP           | When enabled, the server won't attempt to perform a DNS name lookup on any new connections.                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| INCLUDE_RT_VARS          | Allow for retrieval of runtime environment variables from a running task, unhandled exceptions or timeouts, and lagging tasks via `handle_uncaught_error`, `handle_task_timeout`, and `handle_lagging_task`, respectively. To control automatic inclusion of runtime environment variables, set the INCLUDE_RT_VARS server option. Variables will be added to the end of the stack frame as a map.                                                                                                                                  |
| PCRE_PATTERN_CACHE_SIZE  | Specifies how many PCRE patterns are cached.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| SAFE_RECYCLE             | Change ownership of everything an object owns before recycling it.                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| DEFAULT_THREAD_MODE      | Set the default thread mode for threaded functions.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| TOTAL_BACKGROUND_THREADS | Number of threads created at runtime.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| SAVE_FINISHED_TASKS      | Enabled the `finished_tasks` function and define how many tasks get saved by default.                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| DEFAULT_LAG_THRESHOLD    | The number of seconds allowed before a task is considered laggy and triggers `#0:handle_lagging_task`.                                                                                                                                                                                                                                                                                                                                                                                                                              |
| MAX_LINE_BYTES           | Unceremoniously close connections that send lines exceeding this value to prevent memory allocation panics.                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ONLY_32_BITS             | Switch from 64bits back to 32bits.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| CURL_TIMEOUT | Specify the maximum amount of time a CURL request can take before failing. |

#### Running the Server

The server command line has the following general form:
 	
`./moo [-e] [-f script-file] [-c script-line] [-l log-file] [-m] [-w waif-type] [-O|-o] [-4 ipv4-address] [-6 ipv6-address] [-r certificate-path] [-k key-path] [-i files-path] [-x executables-path] input-db-file output-db-file [-t|-p port-number]`

| Option             | Description                                                                     |
| ------------------ | ------------------------------------------------------------------------------- |
| -v, --version      | current version                                                                 |
| -h, --help         | show usage information and command-line options                                 |
| -e, --emergency    | emergency wizard mode                                                           |
| -l, --log          | redirect standard output to log file                                            |
| -m, --clear-move   | clear the `last_move' builtin property on all objects                           |
| -w, --waif-type    | convert waifs from the specified type (check with typeof(waif) in your old MOO) |
| -f, --start-script | file to load and pass to `#0:do_start_script()'                                 |
| -c, --start-line   | line to pass to `#0:do_start_script()'                                          |
| -i, --file-dir     | directory to look for files for use with FileIO functions                       |
| -x, --exec-dir     | directory to look for executables for use with the exec() function              |
| -o, --outbound     | enable outbound network connections                                             |
| -O, --no-outbound  | disable outbound network connections                                            |
| -4, --ipv4         | restrict IPv4 listeners to a specific address                                   |
| -6, --ipv6         | restrict IPv6 listeners to a specific address                                   |
| -r, --tls-cert     | TLS certificate to use                                                          |
| -k, --tls-key      | TLS key to use                                                                  |
| -t, --tls-port     | port to listen for TLS connections on (can be used multiple times)              |
| -p, --port | port to listen for connections on (can be used multiple times)

The emergency mode switch (-e) may not be used with either the file (-f) or line (-c) options.

Both the file and line options may be specified. Their order on the command line determines the order of their invocation.

Examples:
./moo -c '$enable_debugging();' -f development.moo Minimal.db Minimal.db.new 7777
./moo Minimal.db Minimal.db.new

> Note: A full list of arguments is now available by supplying `--help`.

> Note: For both the -c and -f arguments, the script content is passed in the args built-in variable. The server makes no assumptions about the semantics of the script; the interpretation of the script is the verb`s responsibility. Like Emergency Wizard Mode, the verb is called before starting any tasks or doing the initial listen to accept connections.

#### Emergency Wizard Mode
Emergency Wizard Mode

This is a mode that allows you to enter commands on standard input to examine objects or evaluate arbitrary code with wizard permissions in order to, e.g., blank out a forgotten wizard password or repair a database having a broken $do_login_command verb that otherwise would not allow anyone to connect.

When you start the server and supply the -e command line option, the database will load and you will then see a prompt indicating the identity of the wizard whose permissions you are using and the current state of the debug flag, e.g., one of


MOO (#2):
MOO (#2)[!d]:

the latter version of the prompt indicating that the debug flag is unset, and thus that errors will be returned rather than raised, as when you unset the d flag on a verb.

The following commands are available in Emergency Mode:

;expression
;;statements

Evaluate expression or statements, print the expression result or the statement return value.

Note that expression or statement can be omitted, in which case you will be prompted for multiple lines of input, as for the .program command. Type a period on a line by itself to finish.

Also note that no background code, whether resulting from fork statements or suspend() calls, will run until after the Emergency Mode is exited.
program object:verb

Set the code of an existing verb.
list object:verb

List the code of an existing verb.
disassemble object:verb

List the internal form of an existing verb.
debug

Toggle the debug flag.
wizard #objectid

Execute future commands as wizard #objectid, which must be an existing player object with `.wizard==1`.
continue

Exit the emergency mode, continuing with normal start-up. That is, the server will perform the initial listen and start accepting connections.
quit

Exit the emergency mode, save the database and shut down the server.
abort

Exit the emergency mode, and shut down the server without saving the database. This is useful for if you make a mistake
help

Print the list of commands. 

Note that output from wizard mode commands appears on the server`s standard output stream (stdout) and thus can be redirected independently of the log messages if those are being written to the standard error stream (stderr, i.e., if -l has not been specified on the command line).

Also note that unless the server has been compiled to use the NP_SINGLE networking variant, Emergency Wizard Mode is the only use of the server`s standard input and output streams. 
