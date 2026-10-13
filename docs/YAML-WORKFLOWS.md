# Native YAML workflow actions

A `.task` runs without a language SDK. Each step declares exactly one `run`,
`exec`, or `file` action. These primitives support installation recipes while
keeping application publishing and remote transport in the consuming project.

## Named results and guards

An optional `id` names a step's execution-local result. References use
`{result: step.field}`; nested JSON fields use further dots. IDs must be unique,
and only earlier steps can be referenced. A skipped step has no result: referencing
it fails rather than supplying a misleading default.

`when` accepts one declared boolean parameter or one boolean result. There is no
expression evaluator, interpolation in arguments, or automatic conversion of text
to booleans. Existing parameter guards remain supported.

```yaml
steps:
  - id: previous
    file: {operation: read, path: service.conf, optional: true}
  - id: configuration
    file:
      operation: render
      path: service.conf
      source: templates/service.conf
      values: {port: "8080"}
      expected-sha256: {result: previous.sha256}
  - run: restart-service
    when: {result: configuration.changed}
```

Results never update project settings or inherited parameters. A `run` step can
pass scalar result, parameter, setting, and environment references through
`with`. Child task results are not exported to the parent; `run` supplies only
`exit-code`.

## File actions

Paths are absolute or relative to the selected project's root. Exact references
are accepted for path, source, content, replacement, and expected hash. File
actions reject linked paths, Windows reparse points, and files larger than 16 MiB.

| Operation | Additional keys | Result fields |
| --- | --- | --- |
| `read` | `optional`, `format: text\|json` | `path`, `exists`, `content`, `sha256`; JSON also supplies `data` |
| `write` | `content`, `expected-sha256`, `mode` | `path`, `exists`, `changed`, `sha256` |
| `render` | `source`, `values`, `expected-sha256`, `mode` | Same as write |
| `replace` | `find`, `replacement`, required `expected-sha256`, `mode` | Same as write |
| `copy` | `source`, `expected-sha256`, `mode` | Same as write |
| `directory` | `mode` | `path`, `exists`, `changed` |

An optional missing read returns `exists: false`, empty content, and
`sha256: missing`. Supply that sentinel to require creation of a new file.

Changing an existing file requires its observed SHA-256. Replacing identical bytes
is a no-op and preserves its modification time. POSIX permission changes also
count as changes. Writes stage on the destination filesystem, verify the guard
again, and publish atomically. New files never overwrite a concurrently created
file. A cooperating writer lock prevents simultaneous dotask writes; an interrupted
writer's lock requires inspection before removal. Guards do not provide a security
boundary against non-cooperating filesystem writers.

Text updates preserve UTF-8/UTF-8 BOM and BOM-marked UTF-16 LE/BE, existing LF/CRLF
line endings, and final-newline policy. Mixed line endings and undecodable input
fail. `copy` preserves source bytes. New text uses UTF-8; its supplied text
determines line endings and final newline. Existing permissions are preserved
unless `mode` explicitly selects a quoted POSIX mode such as `"0640"`. POSIX
modes are available only on Unix. Directory mode applies only when creating the
destination directory.

Templates use `{{name}}` placeholders and an explicit `values` mapping. Missing
or unclosed placeholders fail. Inserted values are literal and are never reparsed
as templates. `replace` requires exactly one literal match, preserving surrounding
comments and formatting.

## Process environment, output, and timeout

```yaml
steps:
  - id: context
    file:
      operation: read
      path: {environment: DOINSTALL_CONTEXT_FILE}
      format: json
  - id: probe
    exec:
      executable: /usr/bin/curl
      arguments: ["--fail", "--silent", "http://127.0.0.1:5000/health"]
      environment:
        APP_ENVIRONMENT: {result: context.data.values.environment}
      capture-output: true
      timeout-seconds: 30
```

`{environment: NAME}` requires an existing UTF-8 environment variable. An `exec`
step's `environment` mapping overrides only those child-process variables.
Arguments remain separate tokens, including spaces, quotes, and option-like values.

`exec` supplies `exit-code`. With `capture-output: true`, UTF-8 stdout is captured
and supplied as `output`, up to 4 MiB; stderr remains attached. Captured results
are not logged by dotask. A nonzero exit stops the group.

`timeout-seconds` accepts an integer from 1 to 86400. A timed-out process returns
124, stops the workflow, and terminates its owned process tree. An already nested
Unix task shares its outer process group: declare the timeout on the outer
controller instead. Nested Unix timeouts fail before starting a child, avoiding
stranded grandchildren. Doinstall's included-tool timeout owns the whole bundled
dotask workflow; its server recipes should use that outer timeout.

Configuration ownership, backups, activation, and recovery belong to the installer.
These file primitives do not roll back arbitrary commands, OS packages, databases,
or certificates.
