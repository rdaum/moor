# Document Processing

mooR provides built-in functions for working with structured documents: parsing HTML from the web, processing XML data, and generating markup for web interfaces.

## HTML Parsing with html_query()

The `html_query()` function extracts data from HTML using a simple tag-and-attribute query syntax. This is useful for web scraping, processing API responses, and extracting metadata.

```moo
html_query(html_string, tag [, attr_filter])
```

### Parameters

- **html_string**: The HTML text to search
- **tag**: Tag name to find (string or symbol)
- **attr_filter**: Optional map of attribute patterns to filter by

### Return Value

Returns a list of maps, one per matching element. Each map contains:
- All attributes from the element as key-value pairs
- A `"text"` key with the element's inner text content (if any)

### Basic Usage

```moo
let html = "<html><head><title>My Page</title></head><body>...</body></html>";

// Find all title tags
let titles = html_query(html, "title");
// => {["text" -> "My Page"]}

// Get the title text
titles[1]["text"]
// => "My Page"
```

### Attribute Filtering

The third argument filters elements by their attributes. Values support glob-style patterns:

| Pattern | Meaning | CSS Equivalent |
|---------|---------|----------------|
| `"exact"` | Exact match | `[attr="exact"]` |
| `"prefix*"` | Starts with | `[attr^="prefix"]` |
| `"*suffix"` | Ends with | `[attr$="suffix"]` |
| `"*contains*"` | Contains | `[attr*="contains"]` |

```moo
// Find all meta tags with property starting with "og:"
let og_tags = html_query(html, "meta", ['property -> "og:*"]);

// Find links to HTTPS URLs
let secure_links = html_query(html, "a", ['href -> "https:*"]);

// Find meta description tag
let desc = html_query(html, "meta", ['name -> "description"]);
```

### Practical Example: Link Previews

Extract OpenGraph metadata for URL previews:

```moo
verb fetch_preview(url)
  response = worker_request('curl, {"GET", url, "", {}});
  {status, headers, body} = response;

  result = ["url" -> url, "title" -> "", "description" -> "", "image" -> ""];

  // Get OpenGraph tags
  og_tags = html_query(body, "meta", ['property -> "og:*"]);
  for tag in (og_tags)
    prop = tag["property"];
    if (prop && index(prop, "og:") == 1)
      key = prop[4..$];
      if (key in {"title", "description", "image"} && tag["content"])
        result[key] = tag["content"];
      endif
    endif
  endfor

  // Fallback to <title> if no og:title
  if (!result["title"])
    titles = html_query(body, "title");
    if (length(titles) > 0)
      result["title"] = titles[1]["text"];
    endif
  endif

  return result;
endverb
```

## XML Parsing with xml_parse()

The `xml_parse()` function converts XML strings into MOO data structures. Unlike `html_query()`, it requires well-formed XML input.

```moo
xml_parse(xml_string [, result_type [, tag_map]])
```

### Parameters

- **xml_string**: The XML text to parse
- **result_type**: Output format (`TYPE_LIST`, `TYPE_MAP`, or `TYPE_FLYWEIGHT`)
- **tag_map**: For flyweight format, maps tag names to delegate objects

### List Format (default)

Returns nested lists: `{"tag", {"attr", "value", ...}, ...content...}`

```moo
let xml = "<div class='main' id='content'>Hello <b>World</b></div>";
let result = xml_parse(xml);

// result = {"div", {"class", "main"}, {"id", "content"},
//           "Hello ", {"b", "World"}}
```

### Map Format

Same structure, but attributes are stored in a map:

```moo
let result = xml_parse(xml, TYPE_MAP);

// result = {"div", ["class" -> "main", "id" -> "content"],
//           "Hello ", {"b", [], "World"}}
```

### Flyweight Format

Returns flyweight objects that can have verbs called on them:

```moo
let tag_map = ["div" -> $html_div, "b" -> $html_bold];
let result = xml_parse(xml, TYPE_FLYWEIGHT, tag_map);

// result is a flyweight with $html_div as delegate
result:render();  // calls $html_div:render(result)
```

Without a tag_map, the parser looks for system objects named `$tag_<tagname>`.

## Generating XML with to_xml()

The `to_xml()` function converts MOO data structures into XML strings:

```moo
to_xml(structure [, tag_map])
```

### From List Format

```moo
let element = {"div", {"class", "container"},
               "Hello ", {"span", "World"}};
let xml = to_xml(element);
// => "<div class=\"container\">Hello <span>World</span></div>"
```

### From Map Format

```moo
let element = {"div", ["class" -> "container", "id" -> "main"],
               "Hello ", {"span", [], "World"}};
let xml = to_xml(element);
// => "<div class=\"container\" id=\"main\">Hello <span>World</span></div>"
```

### Building HTML Programmatically

```moo
let profile = {"div", ["class" -> "profile"],
    {"img", ["src" -> player.avatar, "alt" -> "Avatar"]},
    {"h2", [], player.name},
    {"p", ["class" -> "bio"], player.description}
};
return to_xml(profile);
```

## JSON Processing

mooR also provides JSON parsing and generation:

### parse_json()

Converts a JSON string into MOO values:

```moo
let data = parse_json("{\"name\": \"Alice\", \"score\": 42}");
// => ["name" -> "Alice", "score" -> 42]
```

JSON types map to MOO as:
- Objects become maps
- Arrays become lists
- Strings, numbers, booleans map directly
- `null` becomes the string `"null"`

### generate_json()

Converts MOO values to JSON strings:

```moo
let json = generate_json(["name" -> "Alice", "scores" -> {10, 20, 30}]);
// => "{\"name\":\"Alice\",\"scores\":[10,20,30]}"
```

## Choosing the Right Tool

| Task | Function |
|------|----------|
| Extract data from web pages | `html_query()` |
| Parse well-formed XML/XHTML | `xml_parse()` |
| Generate HTML/XML output | `to_xml()` |
| Work with JSON APIs | `parse_json()` / `generate_json()` |
