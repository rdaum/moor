# Document Creation and XML Processing

mooR provides powerful built-in functions for working with structured documents, particularly XML. These functions allow
you to parse XML (or well-formed HTML) from external sources and generate XML or HTML for web interfaces, APIs, and data
exchange.

## Overview

The document processing system in mooR supports multiple data formats, but provides special support for XML.

- **List format** - Simple nested lists that mirror XML structure.
- **Map format** - Same as above, but attributes are stored in a map for easier reading
- **Flyweight objects** - Uses mooR's flyweight system to represent XML elements as objects, allowing you to call verbs
  on them.

All formats can represent the same XML structure, but each has different advantages depending on your use case.

## Parsing XML with xml_parse()

The `xml_parse()` function converts XML strings into MOO data structures. The function signature is:

```moo
xml_parse(xml_string, [result_type, [tag_map]])
```

### Parameters

- **xml_string**: The XML text to parse
- **result_type**: The MOO literal type code for the result:
    - LIST - Returns nested lists
    - MAP - Lists where attributes are stored in a map
    - FLYWEIGHT - Returns flyweight objects (requires flyweights enabled)
- **tag_map**: Optional map for flyweight format only, mapping tag names to object references

### List Format (Type LIST)

List format represents XML as nested lists following the pattern:
`{"tag_name", {"attr_name", "attr_value"}, ...content...}`

```moo
let xml = "<div class='container' id='main'>Hello <span>World</span></div>";
let result = xml_parse(xml); // LIST format is the default

// result = {
//   {"div", {"class", "container"}, {"id", "main"}, "Hello ", {"span", "World"}}
// }
```

### Map Format (Type MAP)

Map format is the same as list format, but uses a map for attributes:

```moo
let xml = "<div class='container'>Hello <span>World</span></div>";
let result = xml_parse(xml, MAP);

// result = {
//   "div",
//   ["class" -> "container"],
//   "Hello ",
//   {"span", [], "World"}
// }
```

### Flyweight Format (Type FLYWEIGHT)

Flyweight format uses mooR's special flyweight objects (requires flyweight support enabled):

```moo
let xml = "<div class='container'>Hello World</div>";
let result = xml_parse(xml, 15);

// result = {< $tag_div, [class -> "container"], {"Hello World"} >}
```

**Advantages of flyweight format:**

- Can call verbs on the resulting objects
- Integrates with mooR's object system

**Tag resolution:**
Without a tag_map, the parser looks for objects named `$tag_tagname` (e.g., `$tag_div`, `$tag_span`).
With a tag_map, you can specify custom object mappings:

```moo
let tag_map = ["div" -> $my_div_handler, "span" -> $my_span_handler];
let result = xml_parse(xml, 15, tag_map);
```

## Generating XML with to_xml()

The `to_xml()` function converts MOO data structures into XML strings:

```moo
to_xml(data_structure, [tag_map])
```

### Converting List Format to XML

```moo
// Simple element
let element = {"div", {"class", "container"}, "Hello World"};
let xml = to_xml(element);
// Returns: "<div class=\"container\">Hello World</div>"

// Nested structure
let page = {"html",
    {"head", {"title", "My Page"}},
    {"body", {"class", "main"},
        {"h1", "Welcome"},
        {"p", "This is a paragraph."},
        {"div", {"id", "footer"}, "Copyright 2024"}
    }
};
let html = to_xml(page);
```

### Converting Map Format to XML

```moo
// The new map format is a list where the first element is the tag name,
// the second element is a map of attributes, and remaining elements are content
let element = {"div",
               ["class" -> "container", "id" -> "main"],
               "Hello ",
               {"span", [], "World"}
              };
let xml = to_xml(element);
// Returns: "<div class=\"container\" id=\"main\">Hello <span>World</span></div>"
```

### Mixed Formats

You can mix flyweights, lists, and maps in the same structure:

```moo
// A list containing flyweights and other lists
let mixed = {"div",
    < $my_header, [title -> "Page Title"] >,
    {"p", "Regular paragraph"},
    ["tag" -> "footer", "content" -> {"End of page"}]
};
let xml = to_xml(mixed);
```

## Practical Examples

### Building HTML for Web Interfaces

```moo
let profile = {"div", {"class", "profile-card"},
    {"img", {"src", player.avatar_url}, {"alt", "Avatar"}},
    {"h2", player.name},
    {"p", {"class", "bio"}, player.description},
    {"div", {"class", "stats"},
        {"span", "Level: " + tostr(player.level)},
        {"span", "Score: " + tostr(player.score)}
    }
};
return to_xml(profile);
```

### Processing API Responses

```moo
// Parse weather API XML response
let weather_data = xml_parse(xml_response, 10);

for data in (weather_data)
    if (data["tag"] == "current")
        let temp = data["attributes"]["temperature"];
        let humidity = data["attributes"]["humidity"];

        player:tell("Current temperature: ", temp, "°F");
        player:tell("Humidity: ", humidity, "%");
    endif
endfor
```

### Form Generation

```moo
let form_elements = {"form", {"method", "POST"}};

for field in (fields)
    let input = {"div", {"class", "field"},
        {"label", field.label},
        {"input", {"type", field.type}, {"name", field.name}}
    };
    form_elements = {@form_elements, input};
endfor

form_elements = {@form_elements, {"button", {"type", "submit"}, "Submit"}};
return to_xml(form_elements);
```
