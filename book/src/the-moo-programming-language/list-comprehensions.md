# List Comprehensions

Sometimes you need to create a new list by doing something to every item in an existing list. Maybe you want to double all the numbers, or add "Hello" to the front of every name. In most programming situations, you'd write a loop to go through each item one by one.

List comprehensions give you a more direct way to say "make me a new list where each item is transformed in this specific way." Think of it as a recipe for building lists: you specify what goes in, what transformation to apply, and you get a new list out.

## Basic Syntax

The pattern for a list comprehension looks like this:

```moo
{ what_to_do_with_each_item for each_item in (source_list) }
```

Let's break this down:
- `source_list` is where your original data comes from
- `each_item` is what you call each piece of data as you work with it
- `what_to_do_with_each_item` is the transformation you want to apply
- The curly braces `{}` tell MOO you're creating a new list

## Your First List Comprehension

Let's start with a simple example. Say you have a list of numbers and want to double each one:

```moo
let numbers = {1, 2, 3, 4, 5};
let doubled = { x * 2 for x in (numbers) };
// Result: {2, 4, 6, 8, 10}
```

Here's what happened: MOO took each number from the `numbers` list, called it `x`, doubled it with `x * 2`, and put the result in a new list.

You can do any kind of transformation. Here's how to add a greeting to each name:

```moo
let names = {"alice", "bob", "charlie"};
let greetings = { "Hello, " + name for name in (names) };
// Result: {"Hello, alice", "Hello, bob", "Hello, charlie"}
```

### Working with More Complex Data

You can also work with lists that contain other lists (we call these "nested" lists). For example, if you have student information stored as pairs of name and score:

```moo
let students = {
    {"Alice", 85},
    {"Bob", 92},
    {"Charlie", 78}
};
```

You can extract just the names by asking for the first item in each pair:

```moo
let names = { student[1] for student in (students) };
// Result: {"Alice", "Bob", "Charlie"}
```

Or you could convert the numeric scores to letter grades:

```moo
let grades = {
    student[2] >= 90 ? "A" | student[2] >= 80 ? "B" | "C"
    for student in (students)
};
// Result: {"B", "A", "C"}
```

This uses MOO's conditional operator (`?` and `|`) to check the score and assign the appropriate letter grade.

## Working with Number Ranges

Sometimes you want to create a list based on a sequence of numbers rather than an existing list. MOO lets you create number ranges using the `[start..end]` syntax, and you can use these with comprehensions too.

For example, to create a list of the first five square numbers:

```moo
let squares = { x * x for x in [1..5] };
// Result: {1, 4, 9, 16, 25}
```

Or to create a list of the first ten even numbers:

```moo
let even_numbers = { x * 2 for x in [1..10] };
// Result: {2, 4, 6, 8, 10, 12, 14, 16, 18, 20}
```

Compare this to the traditional way of doing the same thing with a loop:

```moo
// The old way: write a loop
let squares = {};
for x in [1..5]
    squares = {@squares, x * x};
endfor

// The comprehension way: say what you want directly
let squares = { x * x for x in [1..5] };
```

Both produce the same result, but the comprehension version says exactly what you want: "make a list where each item is a number squared."

## Real-World Examples

Here are some practical ways you might use list comprehensions in your MOO programming:

### Working with Game Objects

Say you have a list of object numbers and want to check which ones are valid:

```moo
let player_ids = {#123, #456, #789};
let valid_players = { valid(obj) ? obj | $nothing for obj in (player_ids) };
```

Or maybe you want to get the names of everything in a room:

```moo
let room_names = { item.name for item in (this.contents) };
```

### Text Processing

You might want to capitalize the first letter of each word:

```moo
let words = {"hello", "world", "moo"};
let capitalized = { tostr(word[1..1]):upper() + word[2..] for word in (words) };
// Result: {"Hello", "World", "Moo"}
```

Or generate HTML for a web page:

```moo
let items = {"apples", "oranges", "bananas"};
let html_items = { "<li>" + item + "</li>" for item in (items) };
```

### Number Crunching

Convert temperatures from Celsius to Fahrenheit:

```moo
let celsius = {0, 10, 20, 30, 40};
let fahrenheit = { (c * 9/5) + 32 for c in (celsius) };
// Result: {32, 50, 68, 86, 104}
```

## Why Comprehensions Can Be Faster

Comprehensions often run faster than loops that do the same thing. Here's why:

When you use a loop and keep adding to a list like this:

```moo
// This approach rebuilds the list each time
let result = {};
for x in [1..1000]
    result = {@result, x * 2};
endfor
```

MOO has to create a new, bigger list every time through the loop. That's 1000 list creations!

But with a comprehension:

```moo
// This approach creates the list once
let result = { x * 2 for x in [1..1000] };
```

MOO can figure out how big the final list will be and create it all at once. Much faster!

## When Should You Use Comprehensions?

**Comprehensions are great when:**
- You want to do the same thing to every item in a list
- The transformation is straightforward (not too many complicated steps)
- You want code that clearly shows your intent
- You're working with lots of data and want better performance

**Stick with regular loops when:**
- You need to stop early based on some condition (like `break`)
- You're doing things other than just building a list (like sending messages to players)
- The logic is complex and would be hard to read in one line
- You're working with multiple lists at the same time

## Using Comprehensions with Other mooR Features

List comprehensions work well with other parts of the mooR language. Here are some useful combinations:

### Creating Maps from Lists

You can use comprehensions to build maps (key-value collections) from existing data:

```moo
let coords = {[1, 2], [3, 4], [5, 6]};
let points = { ["x" -> coord[1], "y" -> coord[2]] for coord in (coords) };
```

This takes coordinate pairs and turns them into maps with "x" and "y" keys.

### Using Functions with Comprehensions

Comprehensions work particularly well with [Functions and Lambdas](./lambda-functions.md). You can create small functions that describe transformations, then use them in comprehensions for clean, reusable code:

```moo
// First, create some small functions that do specific transformations
let double = {x} => x * 2;
let square = {x} => x * x;
let greet = {name} => "Hello, " + name;

// Now use them in comprehensions
let numbers = [1..5];
let doubled = { double(x) for x in numbers };     // {2, 4, 6, 8, 10}
let squared = { square(x) for x in numbers };     // {1, 4, 9, 16, 25}

let names = {"Alice", "Bob", "Charlie"};
let greetings = { greet(name) for name in (names) };
// {"Hello, Alice", "Hello, Bob", "Hello, Charlie"}
```

This approach lets you define a transformation once and use it in multiple places.

**For more complex tasks, you can use named functions:**
```moo
// A function that formats player information nicely
fn format_player_info(player)
    return player.name + " (Level " + player.level + ")";
endfn

// Use it in a comprehension to format a whole list of players
let formatted_players = { format_player_info(p) for p in (party_members) };
```

This keeps your comprehension simple and readable while allowing complex logic in the function.

### Calling Methods on Values

mooR lets you call methods directly on values (like strings or numbers). This works great with comprehensions:

```moo
let strings = {"hello", "world", "moo"};
let reversed = { s:reverse() for s in (strings) };
// Result: {"olleh", "dlrow", "oom"}
```

This takes each string and calls its `reverse()` method to flip it backwards.

## Summary

List comprehensions give you a clear, direct way to create new lists by transforming existing data. They're often faster than loops and make your code more readable by clearly expressing your intent. As you get comfortable with them, you'll find they make many common programming tasks much simpler.

Whether you're processing game data, transforming user input, or crunching numbers, comprehensions help you write code that says exactly what you want to accomplish.
