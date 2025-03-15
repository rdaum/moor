# The MOO Programming Language

MOO stands for "MUD, Object Oriented." MUD, in turn, has been said to stand for many different things, but I tend to think of it as "Multi-User Dungeon" in the spirit of those ancient precursors to MUDs, Adventure and Zork.

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
