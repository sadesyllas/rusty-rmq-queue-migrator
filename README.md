# RabbitMQ Queue Migrator

A Rust command-line tool that moves messages from queues in one RabbitMQ virtual host to **existing, same-named queues** in another. It discovers queues through the source Management API, consumes messages over AMQP, republishes their bodies and AMQP properties to the destination, and acknowledges source deliveries after receiving a publisher-confirm result.

The tool moves messages, not RabbitMQ topology. It does not create queues, exchanges, bindings, users, or policies.

## Requirements

- Rust with support for the 2024 edition (Rust 1.85 or newer) to build the project.
- Network access to both brokers' AMQP ports (5672 by default) and the source broker's Management API (15672 by default).
- The RabbitMQ management plugin enabled on the source, with credentials allowed to list queues in the source virtual host.
- AMQP credentials allowed to consume and acknowledge source messages, and to publish to the destination queues. The destination queues must already exist under the same names in the destination virtual host.

AMQP connections use `amqp://`. The source Management API uses HTTP unless `--source-mgmt-use-ssl` selects HTTPS. There is no AMQPS option in the current CLI.

## Build and run

```sh
cargo build --release
./target/release/rusty-rmq-queue-migrator --help
```

Set passwords through `SOURCE_PASSWORD` and `DEST_PASSWORD`, pass the corresponding CLI options, or omit them to get terminal prompts. For an interactive migration:

```sh
./target/release/rusty-rmq-queue-migrator \
  --source-host source.example.com \
  --dest-host destination.example.com \
  --source-username migrator \
  --dest-username migrator \
  --source-vhost / \
  --dest-vhost /
```

The program first displays the queues and their initial ready-message counts, then waits for Enter before moving messages. Use `--only-preflight` to check connectivity, discover eligible queues, and verify destination queues without starting consumers:

```sh
./target/release/rusty-rmq-queue-migrator \
  --source-host source.example.com \
  --dest-host destination.example.com \
  --queue-regex-filter '^orders\.' \
  --only-preflight
```

The examples prompt for both passwords. Avoid putting passwords directly on a shared shell command line, where they may be recorded in shell history or visible to other local processes.

## How it works

1. Parse CLI options and environment variables, prompt for missing passwords, configure logging, and connect to both brokers over AMQP. During a migration, identical source and destination host, AMQP port, and virtual host values are rejected to prevent a loop.
2. Query the **source** Management API at `/api/queues/{vhost}` using pages of 500. If supplied, the queue-name regex is sent to the API as `name` with `use_regex=true`. Requests are retried according to the API options, and there is a one-second pause after each successful page.
3. Keep queues whose `messages_ready` count is greater than zero. For each, passively declare the same name on the destination to verify that a queue exists. Any failed check aborts the run. If there are no eligible queues, exit successfully.
4. Log the eligible queues and their initial ready-message total. `--only-preflight` exits here; otherwise the program waits for Enter.
5. Start up to `--max-parallel-workers` queue workers. Each queue worker starts `--queue-parallelism` consumers, with `--prefetch-count` applied to each source channel. Every consumer uses its own source and destination channel.
6. For each delivery, publish its body and copied AMQP properties to the destination's **default exchange**, using the queue name as the routing key. Publisher confirms are enabled on the destination channel. After a successful confirm *result*, the source delivery is acknowledged; a confirm error causes an attempt to negatively acknowledge and requeue it. A consumer stops after `--consume-timeout` seconds without a delivery or when its stream ends.
7. Close the connections and log elapsed time, the **initially discovered** ready-message total, and the number of queue workers that returned errors.

Because consumers stop on an idle timeout rather than at the initial count, they may also move messages that arrive after discovery. The progress-bar target and final “Total messages moved” line use the initial ready-message count; neither is a verified count of successful destination deliveries.

## CLI reference

Every option below uses a `--long-name`. Environment variables are read by Clap; an explicit CLI value takes precedence. Values in the Default column apply when neither source supplies a value.

| Option | Environment variable | Default | Purpose |
| --- | --- | --- | --- |
| `--source-host <HOST>` | `SOURCE_HOST` | `localhost` | Source AMQP and Management API hostname. |
| `--source-port <PORT>` | `SOURCE_PORT` | `5672` | Source AMQP port. |
| `--source-username <USER>` | `SOURCE_USERNAME` | `guest` | Source AMQP and Management API username. |
| `--source-password <PASSWORD>` | `SOURCE_PASSWORD` | prompt | Source AMQP and Management API password. |
| `--source-vhost <VHOST>` | `SOURCE_VHOST` | `/` | Source virtual host. |
| `--dest-host <HOST>` | `DEST_HOST` | `localhost` | Destination AMQP hostname. |
| `--dest-port <PORT>` | `DEST_PORT` | `5672` | Destination AMQP port. |
| `--dest-username <USER>` | `DEST_USERNAME` | `guest` | Destination AMQP username. |
| `--dest-password <PASSWORD>` | `DEST_PASSWORD` | prompt | Destination AMQP password. |
| `--dest-vhost <VHOST>` | `DEST_VHOST` | `/` | Destination virtual host. |
| `--queue-regex-filter <REGEX>` | `QUEUE_REGEX_FILTER` | all queues | Server-side queue-name regex on the source Management API. |
| `--max-parallel-workers <N>` | `MAX_PARALLEL_WORKERS` | logical CPU count | Maximum queue workers running at once. |
| `--queue-parallelism <N>` | `QUEUE_PARALLELISM` | `1` | Consumers per queue worker. |
| `--prefetch-count <N>` | `PREFETCH_COUNT` | `1000` | Source-channel prefetch count for each consumer. |
| `--consume-timeout <SECONDS>` | `CONSUME_TIMEOUT` | `5` | Idle time before a consumer assumes its queue is empty. |
| `--source-mgmt-port <PORT>` | `SOURCE_MGMT_PORT` | `15672` | Source Management API port. |
| `--source-mgmt-use-ssl` | `SOURCE_MGMT_USE_SSL` | `false` | Use HTTPS for the source Management API. The flag takes no CLI value; the environment variable accepts a boolean. |
| `--api-retries <N>` | `API_RETRIES` | `3` | Additional attempts after an initial failed Management API request. |
| `--api-retry-delay-secs <SECONDS>` | `API_RETRY_DELAY_SECS` | `5` | Pause between Management API attempts. |
| `--log-file-path <PATH>` | `LOG_FILE_PATH` | unset | Write logs to this file as well as stdout. |
| `--log-level <LEVEL>` | `LOG_LEVEL` | `INFO` | Minimum log level: `OFF`, `ERROR`, `WARN`, `INFO`, `DEBUG`, or `TRACE`. |
| `--only-preflight` | — | `false` | Run discovery and destination checks, then exit before migration. |
| `-h`, `--help` | — | — | Show generated CLI help. |
| `-V`, `--version` | — | — | Show package version. |

Port and prefetch values are unsigned 16-bit integers; retry counts are unsigned 32-bit integers. `--max-parallel-workers` is a nonnegative integer. Use positive values for worker counts, prefetch, and timeout: `--max-parallel-workers 0` can wait indefinitely for a permit, and `--queue-parallelism 0` starts no consumers while reporting that queue as finished.

## Delivery behavior and limitations

- Messages are routed by queue name through the destination default exchange. Original exchange and routing-key information is not replayed, and destination bindings or policies are not migrated. Message bodies and AMQP properties are copied.
- Source messages are manually acknowledged after awaiting the destination publisher confirm. This can produce duplicates if the destination accepted a message but its source acknowledgement fails; the code logs an acknowledgement failure.
- The current confirm handling treats **any** successful confirm future result as publish success, including a broker `Nack` result. Publishing also uses the default non-mandatory mode, so an unroutable message may be dropped without a returned-message check. Review this behavior before relying on the tool for loss-sensitive migrations.
- A publish operation that fails before a confirm is obtained exits that consumer with an error; the explicit requeue path handles errors returned while awaiting a confirm. Unexpected consumer stream errors are logged and end the loop without returning a worker error.
- Queue order is not guaranteed when multiple consumers work on one queue. Running producers can add messages during migration, so an idle timeout is the stopping condition rather than a fixed snapshot boundary.
- Queue-worker errors are logged and counted, but the process currently returns success after the completion summary even when that count is nonzero. Check logs and destination counts before treating a run as complete.
- The connection URI percent-encodes virtual hosts but inserts usernames and passwords directly. Reserved URI characters in credentials may prevent AMQP connection. Source and destination AMQP connections do not support TLS through the current options.

## Stack and source layout

This is a single Rust 2024-edition binary built with Cargo. [Tokio](https://crates.io/crates/tokio) runs asynchronous tasks and timers; [Lapin](https://crates.io/crates/lapin) handles AMQP; [Reqwest](https://crates.io/crates/reqwest) and Serde handle Management API requests and JSON; [Clap](https://crates.io/crates/clap) defines the CLI and environment support. Fern and `log` provide logging, Indicatif displays progress, `rpassword` prompts for credentials, and `uuid` generates consumer tags. `anyhow` and `thiserror` represent application errors.

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Startup, preflight, queue-worker scheduling, progress, and completion. |
| `src/config.rs` | CLI options, defaults, and environment-variable mappings. |
| `src/rabbitmq/api.rs` | Paginated source queue discovery and Management API retries. |
| `src/rabbitmq/connector.rs` | AMQP connection URI and connection creation. |
| `src/worker.rs` | Per-queue consumers, publishing, confirms, and source acknowledgements. |
| `src/error.rs` | Worker and integration error types. |

Run `cargo check` to verify the project builds without producing a release binary.

## Build downloadable GitHub release assets

Build on each operating system and upload the resulting archive to the **same GitHub Release**. The version in each filename should match the release tag and the version in `Cargo.toml`. Use a clean checkout of that tag for both builds so the archives contain the same source revision.

### macOS (Apple Silicon)

On an Apple Silicon Mac, from the repository root:

```sh
cargo build --release --locked
mkdir -p dist/macos-arm64
cp target/release/rusty-rmq-queue-migrator README.md dist/macos-arm64/
tar -czf dist/rusty-rmq-queue-migrator-v0.1.0-aarch64-apple-darwin.tar.gz \
  -C dist/macos-arm64 rusty-rmq-queue-migrator README.md
```

The archive is in `dist/`. This machine's native Rust target is `aarch64-apple-darwin`; an Intel Mac needs a separate `x86_64-apple-darwin` build and filename. `dist/` is ignored by Git because compiled binaries belong in release assets, not source control.

### Windows (PowerShell, x64)

From the same tagged source revision on an x64 Windows machine:

```powershell
cargo build --release --locked
New-Item -ItemType Directory -Force dist/windows-x64 | Out-Null
Copy-Item target/release/rusty-rmq-queue-migrator.exe, README.md dist/windows-x64/
Compress-Archive -Path dist/windows-x64/* `
  -DestinationPath dist/rusty-rmq-queue-migrator-v0.1.0-x86_64-pc-windows-msvc.zip `
  -Force
```

Check `rustc -vV` for the actual `host` target triple and adjust the archive name if it differs. Windows Rust builds using the MSVC target require the corresponding Microsoft C++ build tools.

### Upload both assets

On GitHub, open **Releases → Draft a new release**, choose or create a tag such as `v0.1.0`, and upload the macOS `.tar.gz` archive. Save it as a draft if the Windows archive is still pending; edit the same release to add the Windows `.zip`, then publish it. GitHub's automatic “Source code” downloads contain source only; the archives above are the executable downloads. See [GitHub's release instructions](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository).

The current [delivery limitations](#delivery-behavior-and-limitations) apply to these binaries too, especially the publisher-confirm handling. Resolve them before publishing a release intended for loss-sensitive migrations.
