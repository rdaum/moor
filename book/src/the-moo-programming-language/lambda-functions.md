# Functions and Lambdas

mooR lets you create small functions inside your verbs, similar to how you might define functions in other programming
languages. These functions help you organize your code better and avoid repeating yourself.

There are two main types:

- **Named functions** - functions with names that help organize your code
- **Anonymous functions** (also called "lambdas") - unnamed functions that are useful for short, simple operations

> **Fun Fact**: Despite its name suggesting otherwise, the original LambdaMOO never actually had lambda functions! mooR
> brings this useful programming tool to MOO as part of our mission of dragging the future into the past.

## Syntax Forms

Functions in mooR support three different syntax forms, each suited to different use cases:

### Arrow Syntax (Lambdas)

Perfect for simple, one-line anonymous functions:

```moo
{x, y} => x + y                    // Addition function
{name} => "Hello, " + name         // String formatting
{} => random(100)                  // No parameters, returns random number
{x} => x * x                       // Square function
```

### Function Syntax

For more complex logic with multiple statements. These can be anonymous or named:

```moo
// Anonymous function
fn(x, y)
    if (x > y)
        return x;
    else
        return y;
    endif
endfn

// Named function for code organization
fn calculate_damage(attacker, defender)
    let base_damage = attacker.strength * 2;
    let defense = defender.armor;
    let final_damage = max(1, base_damage - defense);
    return final_damage;
endfn
```

### Named Functions for Code Structure

Named functions work much like `def` in Python or `function` in JavaScript - they help organize code within verbs:

```moo
// In a combat verb:
fn calculate_hit_chance(attacker, target)
    let base_chance = 0.5;
    let skill_bonus = attacker.skill / 100.0;
    let dodge_penalty = target.agility / 200.0;
    return base_chance + skill_bonus - dodge_penalty;
endfn

fn apply_damage(target, damage)
    target.health = target.health - damage;
    if (target.health <= 0)
        this:handle_death(target);
    endif
endfn

// Main combat logic becomes much cleaner:
if (random() < calculate_hit_chance(attacker, defender))
    let damage = calculate_damage(attacker, defender);
    apply_damage(defender, damage);
    notify(attacker, "You hit for " + damage + " damage!");
else
    notify(attacker, "You miss!");
endif
```

### Named Recursive Functions

For functions that need to call themselves:

```moo
fn factorial(n)
    if (n <= 1)
        return 1;
    else
        return n * factorial(n - 1);
    endif
endfn

fn fibonacci(n)
    if (n <= 1)
        return n;
    else
        return fibonacci(n - 1) + fibonacci(n - 2);
    endif
endfn
```

## Parameter Patterns

Functions support all the same parameter patterns as regular MOO scatter assignments:

### Required Parameters

```moo
{x, y} => x + y                    // Both x and y must be provided
{name, age} => name + " is " + age // String concatenation
```

### Optional Parameters

Optional parameters default to `0` (false) if not provided:

```moo
{x, ?y} => x + (y || 10)          // y defaults to 0, but we use 10 if not provided
{message, ?urgent} => urgent && ("URGENT: " + message) || message
```

### Rest Parameters

Collect extra arguments into a list:

```moo
{first, @rest} => {first, length(rest)}
{@all} => length(all)              // Count total arguments
```

### Mixed Parameters

You can combine all parameter types:

```moo
{required, ?optional, @rest} => [
    "required" -> required,
    "optional" -> optional || "default",
    "rest" -> rest
]
```

## Remembering Variables (Closures)

One of the most useful features of functions in mooR is that they can "remember" variables from where they were created.
This is called "capturing" variables, and when a function does this, programmers call it a "closure" - but don't worry
about the fancy name, it's simpler than it sounds!

### Basic Example

```moo
let multiplier = 5;
let multiply_by_five = {x} => x * multiplier;  // The function "remembers" multiplier
return multiply_by_five(10);  // Returns 50
```

Even though `multiplier` was created outside the function, the function can still use it!

### Remembering Multiple Things

Functions can remember more than one variable at a time:

```moo
fn create_calculator(operation, initial_value)
    return {x} => {
        if (operation == "add")
            return initial_value + x;
        elseif (operation == "multiply")
            return initial_value * x;
        else
            return x;
        endif
    };
endfn

let add_ten = create_calculator("add", 10);
let multiply_by_three = create_calculator("multiply", 3);

add_ten(5);         // Returns 15
multiply_by_three(4); // Returns 12
```

## Functions Are Like Other Values

In mooR, functions work just like other values (numbers, strings, lists) - you can store them in variables, put them in
properties, and pass them to other functions. This might seem strange at first, but it's actually very useful!

### Storing Functions in Variables

```moo
let add = {x, y} => x + y;
let max_func = fn(x, y) 
    return x > y && x || y; 
endfn;

result = add(5, 3);        // Returns 8
result = max_func(10, 7);  // Returns 10
```

### Storing Functions in Properties

Functions can be stored in object properties and called later:

```moo
this.validator = {input} => length(input) >= 3;
this.formatter = fn(text) 
    return uppercase(text[1]) + lowercase(text[2..$]); 
endfn;

// Later in another verb:
if (this.validator(user_input))
    return this.formatter(user_input);
endif
```

**Important caveat:** Storing functions in properties is rarely what you actually want to do. When you call a function
stored in a property, it doesn't get the `this`, `player`, `caller`, etc. values you'd expect from the object the
property is on - instead it inherits those values from the calling verb. This can lead to confusing behavior.

In most cases, you'll want to use regular verbs instead. Function properties might occasionally be useful for things
like configuration callbacks or data transformation functions, but consider carefully whether a verb wouldn't be
clearer.

### Passing Functions as Arguments

Like Python or JavaScript, you can pass functions to other functions:

```moo
fn process_list(items, transform_func)
    let result = {};
    for item in (items)
        result = {@result, transform_func(item)};
    endfor
    return result;
endfn

let numbers = {1, 2, 3, 4};
let doubled = process_list(numbers, {x} => x * 2);
let strings = process_list(numbers, {x} => "Item: " + tostr(x));
```

### Immediate Invocation

You can call a function immediately after creating it:

```moo
result = ({x} => x * 2)(5);  // Returns 10

// Useful for creating isolated scopes:
let config = (fn()
    let base_url = "https://api.example.com";
    let api_key = "secret123";
    return ["url" -> base_url, "key" -> api_key];
endfn)();
```

## Functions That Use Other Functions

One really useful thing you can do is write functions that take other functions as input. This might sound complicated,
but it's actually quite handy for common tasks like transforming lists of data:

### Map Function

Transform all elements in a list:

```moo
fn map(func, list)
    let result = {};
    for item in (list)
        result = {@result, func(item)};
    endfor
    return result;
endfn

let numbers = {1, 2, 3, 4};
let doubled = map({x} => x * 2, numbers);     // {2, 4, 6, 8}
let squared = map({x} => x * x, numbers);     // {1, 4, 9, 16}
let strings = map({x} => tostr(x), numbers);  // {"1", "2", "3", "4"}
```

### Filter Function

Select elements that match a condition:

```moo
fn filter(pred, list)
    let result = {};
    for item in (list)
        if (pred(item))
            result = {@result, item};
        endif
    endfor
    return result;
endfn

let numbers = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10};
let evens = filter({x} => x % 2 == 0, numbers);        // {2, 4, 6, 8, 10}
let big_numbers = filter({x} => x > 5, numbers);       // {6, 7, 8, 9, 10}
```

### Reduce Function

Combine all elements into a single value:

```moo
fn reduce(func, list, initial)
    let accumulator = initial;
    for item in (list)
        accumulator = func(accumulator, item);
    endfor
    return accumulator;
endfn

let numbers = {1, 2, 3, 4, 5};
let sum = reduce({acc, x} => acc + x, numbers, 0);        // 15
let product = reduce({acc, x} => acc * x, numbers, 1);    // 120
let max = reduce({acc, x} => x > acc && x || acc, numbers, 0); // 5
```

## Event Handling and Callbacks

Lambda functions are perfect for event handling:

```moo
fn on_player_connect(player, callback)
    // Store the callback for later use
    this.connect_callbacks = {@(this.connect_callbacks || {}), callback};
endfn

// Register event handlers
on_player_connect(player, {p} => notify(p, "Welcome to the server!"));
on_player_connect(player, {p} => this:log_connection(p));
```

## Practical Examples

### Data Processing Pipeline

```moo
let data = {"  Alice  ", "  BOB  ", "  charlie  "};

// Clean, normalize, and format names
let clean_names = map({name} => {
    let trimmed = strsub(strsub(name, "^\\s+", ""), "\\s+$", "");
    let lower = lowercase(trimmed);
    return uppercase(lower[1]) + lower[2..$];
}, data);
// Result: {"Alice", "Bob", "Charlie"}
```

### Template Functions

```moo
fn create_template_renderer(template, defaults)
    return {values} => {
        let merged = defaults;
        for key in (keys(values))
            merged[key] = values[key];
        endfor
        let result = template;
        for key in (keys(merged))
            result = strsub(result, "{" + key + "}", tostr(merged[key]));
        endfor
        return result;
    };
endfn

let email_template = create_template_renderer(
    "Hello {name}, your order #{order_id} is ready!",
    ["name" -> "Customer", "order_id" -> "0000"]
);

email_template(["name" -> "Alice", "order_id" -> "1234"]);
// Returns: "Hello Alice, your order #1234 is ready!"
```

### Validation Functions

```moo
fn create_validator(rules)
    return {data} => {
        let errors = {};
        for rule in (rules)
            let field = rule["field"];
            let check = rule["check"];
            let message = rule["message"];
            if (!check(data[field]))
                errors = {@errors, message};
            endif
        endfor
        return errors;
    };
endfn

let user_validator = create_validator({
    ["field" -> "name", "check" -> {x} => length(x) > 0, "message" -> "Name required"],
    ["field" -> "email", "check" -> {x} => "@" in x, "message" -> "Invalid email"]
});

user_validator(["name" -> "Alice", "email" -> "alice@example.com"]);
// Returns: {} (no errors)
```

## Best Practices

### When to Use Functions vs Verbs

**Great for named functions within verbs:**

- Breaking down complex verb logic into smaller, manageable pieces
- Avoiding code duplication within a single verb
- Mathematical calculations or data processing
- Input validation and formatting
- Any logic that would benefit from a descriptive function name

**Great for lambdas (anonymous functions):**

- Short, focused operations
- Event handlers and callbacks
- Data transformation and filtering (map, filter, etc.)
- Function factories and builders
- Functional programming patterns
- One-off operations that don't need names

**Better to use regular verbs for:**

- Functions that need to be called from multiple other verbs
- Complex business logic that forms the core of your application
- Functions that need comprehensive documentation and help
- Code that should be accessible to other objects via inheritance
- Functions that need persistent storage or caching

### Performance Considerations

- Lambda creation is fast, but not free - avoid creating them in tight loops
- Variable capture is done by value, so large data structures are copied
- Recursive lambdas can still hit stack limits like regular MOO recursion (fancy functional languages like Scheme or
  Lisp sometimes offer what's called tail-call elimination, but mooR doesn't)

### Debugging Tips

Stack traces show lambda calls clearly:

- Anonymous lambdas appear as `verb.<fn>`
- Named lambdas appear as `verb.function_name`
- Line numbers point to where the function was declared, not where it was invoked (to find the invocation site, look one level up in the traceback)

## Technical Details

### Variable Capture Semantics

- Variables are captured by **value**, not by reference
- Captured variables are immutable from the lambda's perspective
- Each lambda invocation gets its own copy of captured variables

### Memory Management

- Lambdas are garbage collected when no longer referenced
- Captured variables are freed when the lambda is freed
- Circular references between lambdas are not possible due to pass-by-value capture

### Compatibility

- Lambda functions are a mooR extension and won't work on LambdaMOO
- They can be stored in the database like any other value
- They survive server restarts when stored in properties
- They work seamlessly with existing MOO functions and operators

Lambda functions represent a significant enhancement to MOO's programming capabilities, enabling more expressive and
functional programming patterns while maintaining full compatibility with existing MOO code.