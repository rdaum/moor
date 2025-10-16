# Load Testing Tools

Tools for testing the consistency and performance of the moor transaction system.

## tx-list-append

A workload generator that exercises Jepsen/Elle `list-append` operations against the scheduler to
verify serializability guarantees.

### What it does

- Creates multiple concurrent tasks that randomly read and append to list properties
- Records all operations in EDN format compatible with
  [elle-cli](https://github.com/ligurio/elle-cli)
- Can be used to detect consistency anomalies like write cycles, read skew, and anti-dependency
  violations

### Usage

#### Local Testing with Docker

The easiest way to run the full test including Elle verification:

```bash
# From the workspace root
docker build -f Dockerfile.elle -t moor-elle-test .
docker run moor-elle-test
```

You can customize the workload parameters:

```bash
docker run -e NUM_PROPS=10 -e NUM_CONCURRENT=20 -e NUM_ITERATIONS=50 moor-elle-test
```

To extract the workload file and results:

```bash
docker run --name elle-test moor-elle-test
docker cp elle-test:/output/workload.edn ./
docker cp elle-test:/output/elle-result.txt ./
docker rm elle-test
```

#### Running directly

```bash
cargo run --bin moor-model-checker -- \
    --num-props 5 \
    --num-concurrent-workloads 10 \
    --num-workload-iterations 20 \
    --output-file workload.edn
```

Then check with elle-cli:

```bash
java -jar elle-cli-*-standalone.jar --model list-append workload.edn
```

### Parameters

- `--num-props`: Number of list properties to use (default: 5)
- `--num-concurrent-workloads`: Number of concurrent task streams (default: 20)
- `--num-workload-iterations`: Number of operations per task stream (default: 20)
- `--output-file`: Where to write the EDN workload (default: workload.edn)
- `--db-path`: Database directory (default: temporary)
- `--debug`: Enable debug logging

### How it works

1. Creates a test database with a wizard player object
2. Defines `num-props` list properties on the player object
3. Creates two MOO verbs:
   - `write_workload`: Reads current list values, generates unique random integers, appends them
   - `read_workload`: Reads current list values
4. Spawns `num-concurrent-workloads` concurrent tasks
5. Each task performs `num-workload-executions` random read or write operations
6. Records all operations with timestamps in EDN format
7. Elle analyzes the history for consistency violations

### Expected Result

For a correct serializable implementation, elle-cli should output:

```
workload.edn	true
```

Any other result indicates a consistency bug in the transaction system.
