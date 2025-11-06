# Webhooks

Webhooks allow the MOO server to handle incoming HTTP requests and serve dynamic content directly from MOO code. This
enables the MOO to function as a web application server, serving HTML pages, JSON APIs, or any other HTTP content.

## Enabling Webhooks

Webhooks are enabled via a command-line flag when starting the `web-host`:

Here's an example, although this is something only an administrator would have to be concerned with and you should ask
your admin about:

```bash
./target/release/moor-web-host --enable-webhooks
```

When enabled, all HTTP requests to paths starting with `/webhooks/` will be routed to the MOO's `#0:invoke_http_handler`
verb.

## Request Processing

When an HTTP request is received at a webhook path (e.g., `http://localhost:8080/webhooks/test/friendly`), the following
happens:

1. The request is parsed and converted to MOO data structures
2. A system handler task is created to call `#0:invoke_http_handler`
3. The handler runs with the system object (#0) permissions unless an auth token is provided
4. The handler's return value is converted back to an HTTP response

## Handler Arguments

The `#0:invoke_http_handler` verb receives the following arguments:

```moo
#0:invoke_http_handler(method, path, query_params, headers, body, client_ip)
```

- **`method`**: HTTP method as string ("GET", "POST", "PUT", etc.)
- **`path`**: Request path as string (e.g., "/webhooks/test/friendly")
- **`query_params`**: Query parameters as a list of `{key, value}` pairs
- **`headers`**: HTTP headers as a list of `{key, value}` pairs
- **`body`**: Request body as string (for text) or binary (for binary data)
- **`client_ip`**: Client IP address and port as string (e.g., "127.0.0.1:35194")

### Example Request Data

For a request like:

```bash
curl "http://localhost:8080/webhooks/test/friendly"
```

The handler would receive:

```moo
{"GET", "/webhooks/test/friendly", {},
 {{"accept", "*/*"}, {"user-agent", "curl/8.16.0"},
  {"host", "localhost:8080"}, {"connection", "close"}},
 "", "127.0.0.1:35194"}
```

## Authentication and Permissions

By default, webhook handlers run with system object (#0) permissions. However, you can provide authentication via a
PASETO token in the `Authorization` header:

```bash
curl -H "Authorization: Bearer <paseto_token>" "http://localhost:8080/webhooks/private"
```

When an auth token is provided:

- The handler runs with the permissions of the authenticated player
- The `player` variable in the handler will be the player's object ID
- Without a token, `player` is the system object (#0)

## Response Formats

Webhook handlers can return several types of responses:

### Simple String Response

Return a string to send a plain text response:

```moo
#0:invoke_http_handler   this none this
 1:  return "You sent me: " + toliteral(args);
```

This returns:

- Status code: 200 OK
- Content-Type: text/plain
- Body: The returned string

### Binary Response

Return a binary value to send raw bytes:

```moo
#0:invoke_http_handler   this none this
 1:  return b"SGVsbG8gd29ybGQ=";  "Base64-encoded 'Hello world'"
```

This returns:

- Status code: 200 OK
- Content-Type: application/octet-stream
- Body: The binary data

### Structured Response

Return a list to control the full HTTP response:

```moo
#0:invoke_http_handler   this none this
 1:  return {200, "You sent me: " + toliteral(args), "text/plain", {{"monkey", "paw"}}};
```

The list format is:

- **Index 0**: HTTP status code (integer)
- **Index 1**: Response body (string or binary)
- **Index 2**: Content-Type header (string, optional)
- **Index 3**: Additional headers (list of `{key, value}` pairs, optional)

This would return:

```
HTTP/1.1 200 OK
Content-Type: text/plain
monkey: paw
Content-Length: 190

You sent me: {"GET", "/webhooks/test/friendly", ...}
```

## Use Cases

Webhooks enable many powerful use cases:

### Dynamic Content Serving

```moo
#0:invoke_http_handler   this none this
 1:  {method, path, query, headers, body, client_ip} = args;
 2:  if (path == "/webhooks/help")
 3:      return {200, "<h1>Help Page</h1><p>Welcome to the MOO!</p>", "text/html"};
 4:  elseif (path == "/webhooks/api/users")
 5:      return {200, tojson(users()), "application/json"};
 6:  else
 7:      return {404, "Not found", "text/plain"};
 8:  endif
```

For more sophisticated HTML generation using MOO's XML processing capabilities, see [XML Documents](xml-documents.md).

### Action Triggers

```moo
#0:invoke_http_handler   this none this
 1:  {method, path, query, headers, body, client_ip} = args;
 2:  if (method == "POST" && path == "/webhooks/notify")
 3:      "Parse JSON body and trigger notification"
 4:      notify_all("Webhook notification: " + body);
 5:      return {200, "Notification sent", "text/plain"};
 6:  endif
 7:  return {405, "Method not allowed", "text/plain"};
```

### Form Processing

```moo
#0:invoke_http_handler   this none this
 1:  {method, path, query, headers, body, client_ip} = args;
 2:  if (method == "POST" && path == "/webhooks/contact")
 3:      "Parse form data from body"
 4:      {name, email, message} = parse_form_data(body);
 5:      "Store in database"
 6:      contact_form = create_object(#contact_form);
 7:      contact_form.name = name;
 8:      contact_form.email = email;
 9:      contact_form.message = message;
10:      return {200, "Thank you for your message!", "text/html"};
11:  endif
12:  "Show contact form for GET requests"
13:  return {200, contact_form_html(), "text/html"};
```

## Error Handling

If the handler raises an error or returns an unsupported value type:

- A 500 Internal Server Error is returned
- The error is logged server-side
- No MOO stack trace is exposed to the client

## Performance Considerations

- Webhook handlers have a 30-second timeout
- Long-running handlers will cause HTTP timeouts
- For expensive operations that don't require immediate HTTP response, use `fork()` to run them in the background
- Consider using workers for external API calls that might block or take significant time

### Using fork() for Background Processing

If you need to perform expensive operations (like database cleanup, file processing, or sending notifications) but don't
need to wait for them to complete before returning an HTTP response, use `fork()`:

```moo
#0:invoke_http_handler   this none this
 1:  {method, path, query, headers, body, client_ip} = args;
 2:  if (method == "POST" && path == "/webhooks/process-image")
 3:      "Return immediately and process image in background"
 4:      fork(0)
 5:          "Background task: expensive image processing"
 6:          process_large_image(body);
 7:      endfork
 8:      return {202, "Image processing started", "text/plain"};
 9:  endif
10:  return {405, "Method not allowed", "text/plain"};
```

This pattern allows the HTTP request to complete quickly while the expensive work happens asynchronously.

## Security Best Practices

1. **Validate all inputs** - Don't trust query parameters or headers
2. **Use authentication** for sensitive endpoints
3. **Sanitize output** when returning HTML
4. **Limit resource usage** in handlers
5. **Log suspicious activity** for monitoring

