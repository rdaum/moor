# Variables in MOO

## What are variables?

Variables are like labeled containers that hold values in your MOO programs. Think of them as boxes with names written on them - you can put different things in the box, take things out, and refer to the box by its name.

```moo
// Create a variable called 'player_name' and put a string in it
player_name = "Alice";

// Create a variable called 'score' and put a number in it
score = 100;

// Create a variable called 'inventory' and put a list in it
inventory = {"sword", "shield", "potion"};
```

## MOO is dynamically typed

MOO is what's called a "dynamically typed" language. This means:

- **You don't have to declare what type of value a variable will hold** when you create it
- **Variables can hold any type of MOO value** - strings, numbers, lists, objects, etc.
- **The same variable can hold different types at different times** in your program
- **Type checking happens when your program runs**, not when you write it

### What this means for you:

**The good news:** Writing code is often simpler and more flexible:

```moo
// This is perfectly fine in MOO:
my_var = "Hello";        // my_var holds a string
my_var = 42;             // now my_var holds a number
my_var = {"a", "b"};     // now my_var holds a list
my_var = #123;           // now my_var holds an object reference
```

**The catch:** You can get runtime errors if you make wrong assumptions:

```moo
player_name = "Alice";
score = player_name + 100;    // ERROR! Can't add a string and number
```

This will give you an `E_TYPE` error when your program runs, because MOO can't add a string (`"Alice"`) to a number (`100`).

### Best practices for dynamic typing:

1. **Use descriptive variable names** that hint at what they contain:
   ```moo
   player_count = 5;           // clearly a number
   player_names = {"Alice"};   // clearly a list of names
   current_room = #17;         // clearly an object reference
   ```

2. **Check types when you're unsure**:
   ```moo
   if (typeof(user_input) == STR)
       player:tell("You said: " + user_input);
   else
       player:tell("I expected you to say something!");
   endif
   ```

## Variable scope in mooR

**Variable scope** refers to where in your program a variable can be used. mooR gives you several options for controlling scope, which makes it more powerful than the original LambdaMOO.

### Global scope (verb-wide)

By default, when you create a variable in MOO, it has "global" scope within that verb. This means the variable can be used anywhere in the verb, from the moment it's created until the verb ends.

```moo
// This verb demonstrates global scope
if (player.score > 100)
    high_score_message = "Congratulations!";  // Created inside if block
endif

// But we can use it outside the if block too:
player:tell(high_score_message);  // This works fine
```

### Block scope with `let` and `const`

mooR adds the keywords `let` and `const` to create variables with "block scope." These variables only exist within the block (like inside an `if` statement, `while` loop, or explicit `begin/end` block) where they're created.

#### Using `let` for block-scoped variables:

```moo
if (player.level > 5)
    let bonus_points = player.level * 10;  // Only exists in this if block
    player.score = player.score + bonus_points;
endif

// This would cause an error - bonus_points doesn't exist here:
// player:tell("Bonus: " + tostr(bonus_points));  // ERROR!
```

#### Using `const` for block-scoped constants:

```moo
if (item.type == "weapon")
    const MAX_DAMAGE = 50;  // A constant that can't be changed
    if (item.damage > MAX_DAMAGE)
        item.damage = MAX_DAMAGE;  // Cap the damage
    endif
    // MAX_DAMAGE = 60;  // ERROR! Can't change a const
endif
// MAX_DAMAGE doesn't exist outside the if block
```

### Explicit blocks with `begin/end`

mooR also lets you create explicit blocks using `begin` and `end` keywords. This is useful when you want to limit variable scope without needing an `if` or `while` statement:

```moo
// Global variable
total_score = 0;

begin
    let temp_calculation = player.base_score * 2;
    let bonus = player.achievements * 10;
    total_score = temp_calculation + bonus;
end

// temp_calculation and bonus don't exist here anymore
player:tell("Your total score is: " + tostr(total_score));
```

### The `global` keyword

Sometimes when you're inside a block, you want to explicitly create or modify a global variable. You can use the `global` keyword to be clear about this:

```moo
player_count = 0;  // Global variable

if (new_player_joined)
    global player_count;  // Explicitly refer to the global variable
    player_count = player_count + 1;

    let welcome_message = "Welcome! You are player #" + tostr(player_count);
    player:tell(welcome_message);
endif
```

## Why does scope matter?

Understanding scope helps you write better, cleaner code:

### 1. **Prevents naming conflicts:**
```moo
total = 0;  // Global total

for item in (player.inventory)
    let total = item.value;  // Local total, doesn't interfere
    if (total > 100)
        player:tell(item.name + " is valuable!");
    endif
endfor

// The global 'total' is still 0 here
```

### 2. **Makes code easier to understand:**
```moo
// Bad: unclear scope
if (condition)
    temp_value = calculate_something();
endif
result = temp_value;  // Is temp_value always set?

// Better: clear scope
result = 0;  // Default value
if (condition)
    let temp_value = calculate_something();
    result = temp_value;
endif
```

### 3. **Prevents accidental variable reuse:**
```moo
// Without block scope, this could be confusing:
for i in [1..5]
    // ... do something with i
endfor

for i in [1..10]  // Same variable name, might be confusing
    // ... do something else with i
endfor

// With block scope, each loop has its own 'i':
for let i in [1..5]
    // ... this i only exists in this loop
endfor

for let i in [1..10]
    // ... this is a completely different i
endfor
```

## Common variable patterns

### 1. **Temporary calculations:**
```moo
begin
    let base_damage = weapon.damage;
    let strength_bonus = player.strength / 10;
    let final_damage = base_damage + strength_bonus;

    target.health = target.health - final_damage;
end
```

### 2. **Configuration constants:**
```moo
const MAX_INVENTORY_SIZE = 20;
const STARTING_GOLD = 100;

if (length(player.inventory) >= MAX_INVENTORY_SIZE)
    player:tell("Your inventory is full!");
    return;
endif
```

### 3. **Loop variables:**
```moo
// Process each item in inventory
for let item in (player.inventory)
    if (item.broken)
        player:tell(item.name + " is broken and falls apart!");
        // item only exists within this loop
    endif
endfor
```

## Best practices for variables

1. **Use descriptive names**: `player_health` is better than `ph` or `x`

2. **Use `let` for temporary values** that don't need to exist outside their block

3. **Use `const` for values that shouldn't change** within their scope

4. **Use global scope sparingly** - prefer limiting scope when possible

5. **Initialize variables** with sensible default values:
   ```moo
   message = "";  // Start with empty string
   count = 0;     // Start with zero
   items = {};    // Start with empty list
   ```

6. **Check variable types** when working with user input or uncertain data:
   ```moo
   if (typeof(user_input) == STR && length(user_input) > 0)
       // Safe to work with user_input as a non-empty string
   endif
   ```

Variables are fundamental to MOO programming - master them, and you'll be able to write much more powerful and organized code!
