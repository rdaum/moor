# moor-web-mcp

Web-host backed MCP stdio server for mooR.

## Configuration

Create a JSON config file:

```json
{
    "baseUrl": "http://localhost:8080",
    "defaultCharacter": "programmer",
    "characters": [
        {
            "id": "programmer",
            "username": "programmer",
            "password": "secret",
            "isProgrammer": true,
            "notes": "Default coding character."
        },
        {
            "id": "wizard",
            "username": "wizard",
            "password": "wizard-secret",
            "isWizard": true,
            "notes": "Elevated operations only."
        }
    ]
}
```

## Run

```bash
npm run build --prefix clients/moor-web-mcp
node clients/moor-web-mcp/dist/index.js --config /path/to/moor-web-mcp.json
```
