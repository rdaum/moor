## Working with WAIFs

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

### The WAIF Verb and Property Namespace

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

### Additional Details on WAIFs

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
