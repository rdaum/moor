# MOO Language Statements

Statements are MOO constructs that, in contrast to expressions, perform some useful, non-value-producing operation. For example, there are several kinds of statements, called _looping constructs_, that repeatedly perform some set of operations. Fortunately, there are many fewer kinds of statements in MOO than there are kinds of expressions.

## Errors While Executing Statements

Statements do not return values, but some kinds of statements can, under certain circumstances described below, generate errors. If such an error is generated in a verb whose `d` (debug) bit is not set, then the error is ignored and the statement that generated it is simply skipped; execution proceeds with the next statement.

> Note: This error-ignoring behavior is very error prone, since it affects _all_ errors, including ones the programmer may not have anticipated. The `d` bit exists only for historical reasons; it was once the only way for MOO programmers to catch and handle errors. The error-catching expression and the `try` -`except` statement are far better ways of accomplishing the same thing.

If the `d` bit is set, as it usually is, then the error is _raised_ and can be caught and handled either by code surrounding the expression in question or by verbs higher up on the chain of calls leading to the current verb. If the error is not caught, then the server aborts the entire task and, by default, prints a message to the current player. See the descriptions of the error-catching expression and the `try`-`except` statement for the details of how errors can be caught, and the chapter on server assumptions about the database for details on the handling of uncaught errors.

## Simple Statements

The simplest kind of statement is the _null_ statement, consisting of just a semicolon:

```
;
```

It doesn't do anything at all, but it does it very quickly.

The next simplest statement is also one of the most common, the expression statement, consisting of any expression followed by a semicolon:

```
expression;
```

The given expression is evaluated and the resulting value is ignored. Commonly-used kinds of expressions for such statements include assignments and verb calls. Of course, there's no use for such a statement unless the evaluation of expression has some side-effect, such as changing the value of some variable or property, printing some text on someone's screen, etc.

```
#42.weight = 40;
#42.weight;
2 + 5;
obj:verbname();
1 > 2;
2 < 1;
```

## Statements for Testing Conditions

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

## Statements for Looping

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

## Terminating One or All Iterations of a Loop

Sometimes, it is useful to exit a loop before it finishes all of its iterations. For example, if the loop is used to search for a particular kind of element of a list, then it might make sense to stop looping as soon as the right kind of element is found, even if there are more elements yet to see. The `break` statement is used for this purpose; it has the form

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

## Returning a Value from a Verb

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

## Handling Errors in Statements

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

## Cleaning Up After Errors

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

## Executing Statements at a Later Time

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

then that variable is assigned the _task ID_ of the newly-created task. The value of this variable is visible both to the task executing the fork statement and to the statements in the newly-created task. This ID can be passed to the `kill_task()` function to keep the task from running and will be the value of `task_id()` once the task begins execution.

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
