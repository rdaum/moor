# Configure connections, clients, and workers

Several components take part when someone visits your world. Understanding their jobs helps you find
the right configuration file when something goes wrong.

The standard Cowbell launcher runs two containers. The `moor` container runs a single executable
with the world runtime, network hosts, and enabled workers. The `moor-frontend` container serves
Meadow and forwards its requests to the web host.

```text
Meadow in a browser -> frontend on port 8080 -> web host on port 8081
                                                      |
Terminal client -----------------> telnet on port 8888 |
                                          |           |
                                          v           v
                                           Cowbell world
                                                |
                                       enabled external workers
```

The web and telnet hosts connect people to the same objects and accounts. A worker handles a request
outside the MOO, such as fetching an HTTP resource or reading a Git repository.

## Which file controls what?

The [starting walkthrough](starting.md) uses three environment variables:

| Variable           | What it selects                                                    | Example                                 |
| ------------------ | ------------------------------------------------------------------ | --------------------------------------- |
| `RUN_DIR`          | Persistent storage under the checkout.                             | `run-cowbell`                           |
| `MOOR_CONFIG_FILE` | Server YAML configuration.                                         | `moor-local.yaml`                       |
| `COMPOSE_FILE`     | Compose files, in order, with later files overriding earlier ones. | `docker-compose.yml:compose.local.yaml` |

`moor-local.yaml` controls server features and component settings. Compose supplies paths, published
ports, and several command-line options. Those explicit command-line options take precedence over
the corresponding server settings.

For example, Compose starts the web host on port `8081`, even though `moor-dev.yaml` specifies
`8080`. Port `8080` on your computer belongs to the frontend in this arrangement.

After changing Compose settings, the existing containers need to be recreated.
`docker compose start` only starts the containers already created. Running the launcher applies the
configuration again.

To inspect the configuration Compose will use, run:

```sh
docker compose config
```

This shows substituted environment values and merged files. Keep the output private if your
configuration contains credentials.

## Enable the components a feature needs

The launcher explicitly enables the HTTP/curl and Git workers. An installation started another way
must enable the workers it needs separately.

| Feature                          | Requirements beyond the basic world                                                                           |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Browser access                   | A web host and a Meadow frontend configured to reach it.                                                      |
| Terminal access                  | An enabled telnet host and a reachable listening port.                                                        |
| `@changes` from a Git repository | Git worker, repository access, and an actual wizard for Git fetches.                                          |
| `@changes` from an HTTP bundle   | HTTP/curl worker and access to the bundle URL.                                                                |
| Model requests                   | The request transport, model/provider configuration, credentials, and object configuration used by that tool. |

A missing worker does not mean that a package or object is missing. For example, `@changes` can
exist and accept a fetch request, then report that no worker is available. Read that failure before
changing the package membership or replacing objects.

Enabling a worker does not configure the model tools. See
[language models](../language-models/index.md) for the remaining documentation work in that area.

## Move beyond a local experiment

The local override in the starting walkthrough binds published ports to `127.0.0.1`. Other computers
cannot use those bindings directly. A public installation needs deliberate network addresses,
encrypted browser access, and account configuration.

The general server manual covers
[web deployment](https://timbran.org/book/2.0-dev/html/web-client/deployment.html) and
[server configuration](https://timbran.org/book/2.0-dev/html/the-system/server-configuration.html).
Use those instructions for TLS, reverse proxies, and host settings. Plain telnet does not encrypt
passwords or conversation in transit.

Before inviting visitors, review the supplied accounts, registration setting, welcome text, and
community policy. [Accounts and authority](accounts.md) describes their relationship to Cowbell.

## A second world needs more than another data directory

Changing `RUN_DIR` selects another database, but it does not change the container names or published
ports. The supplied Compose file fixes the names `moor` and `moor-frontend`.

For the restore exercise, stop the original installation before starting the copy. Simultaneous
worlds need distinct container names, ports, and configuration as well as distinct storage. Merely
choosing another Compose project name does not remove the fixed container names.

For split processes or multiple machines, use the server manual's
[clustered deployment guide](https://timbran.org/book/2.0-dev/html/the-system/clustered-deployment.html).
The local storage recipe in this book does not cover that arrangement.
