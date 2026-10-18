# Native YAML workflow actions

A `.task` runs without a language SDK. Each step declares exactly one `run`,
`exec`, `file`, `http`, `json`, `text`, or `foreach` action. Tasks combine these
primitives to describe their own workflows; dotask contains no GitHub or website
synchronization command. Help/completion validate metadata without making requests
or executing actions.

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

## HTTP GET and pagination

```yaml
steps:
  - id: repositories
    http:
      method: GET
      url: https://api.github.com/users/kodybrown/repos
      query: {type: owner, per_page: 100}
      headers: {Accept: application/vnd.github+json}
      response-format: json
      pagination: {mode: link-header, relation: next, max-pages: 100}
      timeout-seconds: 30
```

The result contains `status-code`, `headers` (lowercase names mapped to arrays of
strings), `data`, `pages`, and boolean `found`. Successful responses have
`found: true`. Without pagination, `data` can be any JSON value.
With pagination, each response must be an array; `data` combines all pages in
order. Headers/status describe the final page. Every page must succeed before the
step publishes a result. There are no automatic retries or cached responses.

URLs, header values, and query values accept exact parameter/setting/environment/
result/item references. Query values are encoded as individual scalar values.
The client sends `User-Agent: dotask` along with configured headers. Only GET is
supported. `response-format: json` parses JSON; `response-format: text` returns
the exact UTF-8 response text in `data`. Text responses cannot use pagination.
For known legacy inputs, `fallback-encoding: windows-1252` decodes bytes in that
encoding only if UTF-8 decoding fails. Valid UTF-8 is never reinterpreted. The
fallback is explicit, only supported for text responses, and never inserts
replacement characters. Without it, invalid UTF-8 remains an error.

`optional: true` treats only HTTP 404 as absence: `found: false`, `data: null`,
`pages: 1`, with the response status and headers retained. It does not read or
parse the 404 error body. Guard dependent steps with `when: {result: name.found}`.
Optional requests cannot paginate, so a missing later page can never produce a
partial collection. Other non-2xx status, invalid JSON/UTF-8, invalid links,
transport failure, or timeout stop the workflow. Redirects are not followed. Diagnostics
exclude response bodies, request header values, and runtime URLs.

`timeout-seconds` defaults to 30, accepts 1 through 86400, and covers the entire
HTTP action, including all pages and body reads. Cancellation remains responsive
during DNS/TLS/reads. Total decoded response bytes are limited to 16 MiB. Pagination
defaults to at most 100 pages; `max-pages` accepts 1 through 1000. Next links must
be absolute HTTP(S) URLs with the original scheme and authority; repeated links,
ambiguous next links, and limits fail rather than returning a partial collection.

## JSON filtering and projection

```yaml
steps:
  - id: source
    file: {operation: read, path: repositories.json, format: json}
  - id: originals
    json:
      operation: filter
      input: {result: source.data}
      where: {field: fork, equals: false}
  - id: names
    json:
      operation: select
      input: {result: originals.data}
      fields: {id: id, name: name, url: html_url}
```

Both operations return `data`. `filter` requires an array and retains records
whose selected field equals the supplied value with JSON type preserved (false
differs from `"false"`). `select` constructs an object from a field mapping, or
maps that construction over an input array. Field paths are case-sensitive,
dot-separated object keys or array indexes; an empty projection path selects the
entire input. Missing fields fail; existing JSON nulls remain null. Input and
filter comparison values support exact references. These are bounded data
operations, not general expressions or a script language. Referenced values are
data and are never reinterpreted as another reference.

## Sequential foreach and text formatting

```yaml
steps:
  - id: source
    file: {operation: read, path: repositories.json, format: json}
  - foreach: {items: {result: source.data}, as: repo}
    steps:
      - id: path
        text:
          template: "src/projects/{{name}}.md"
          values: {name: {item: repo.name}}
      - run: update-project
        with:
          path: {result: path.text}
          repository-id: {item: repo.id}
```

`foreach.items` is an array or exact structured reference. `as` declares an item
name; `{item: repo}` selects the whole item, and `{item: repo.name}` selects a
field with the same paths as JSON operations. Loop bodies may contain any action,
including nested loops and ordinary `run` calls. Calls still accept only scalar
parameters through `with`; nulls/objects/arrays are not implicitly serialized.

Steps execute sequentially and stop on the first failure. Outer earlier results
are visible inside a loop; each iteration starts without results from previous
iterations. Body results are unavailable after the loop. Result IDs cannot
shadow an enclosing result; item names cannot shadow an enclosing item. A named
loop publishes only `iterations`, not its body results. Each loop accepts at most
10000 items; nesting is limited to 16 levels. An empty loop does not satisfy
`require_at_least_1_step`. There is no parallel execution, break, or goto.

`text` returns `text`. Its literal `template` uses `{{name}}` placeholders and an
explicit `values` mapping of scalar literals/references. Inserted data is literal
and is never reparsed. Missing placeholders, invalid scalar values, and unclosed
placeholders fail. Arbitrary process arguments still have no interpolation.

## Markdown frontmatter updates

```yaml
steps:
  - file:
      operation: frontmatter
      path: src/projects/fixeol.md
      create-if-missing: true
      initial: {title: FixEOL, summary: "", screenshot: "", hidden: false}
      match-existing: {github.id: 123456}
      fill-empty: {summary: "An optional initial blurb"}
      set:
        github.id: 123456
        github.description: Fixes line endings in files.
        github.archived: false
```

This operation edits YAML frontmatter delimited by `---` at the start of a text
document. It creates a missing file only with `create-if-missing: true`. Existing
documents without a header can gain one without changing their body. A file
beginning with an unclosed header is refused. No filename extension is required.

All four mappings use explicit dot-separated field paths with simple ASCII
letters/digits/underscore/hyphen keys, and accept typed values/references:

| Mapping | Policy |
| --- | --- |
| `match-existing` | Before edits, require each existing non-null field to equal the supplied value; missing fields are allowed. |
| `initial` | Apply only when creating the document. |
| `fill-empty` | Fill absent/null/blank-string fields from non-null, nonblank values; preserve false, zero, and existing collections. |
| `set` | Update exactly these fields, including explicit nulls and false values. |

Policies apply in the order shown. Parent/child paths may not overlap within one
mapping. Unknown fields and all nonblank Markdown body text remain untouched. Existing
block YAML mappings and single-line values are patched without serializing the
document; new values use JSON-compatible YAML, with strings/dates quoted. Untouched
multiline fields remain literal. Changing multiline/block scalar values, traversing
flow mappings, or editing anchored/tagged values/mappings is refused. Convert
those particular fields to simple block YAML manually before retrying. A
nonmapping parent is an error; it is never silently replaced.

### Fill a blank Markdown body

A frontmatter action also returns boolean `body-empty`, measured after the update.
Use it to avoid downloading content for an already-authored document. The optional
`body-if-empty` mapping supplies `content` (a literal string or exact scalar
reference) and optional boolean `strip-first-h1` (default false):

```yaml
steps:
  - file:
      operation: frontmatter
      path: page.md
      body-if-empty:
        content: {parameter: markdown}
        strip-first-h1: true
```

The action checks the current body again when writing. Any non-whitespace body
character, including comments or code, preserves the entire existing body.
Whitespace-only bodies can be filled; a headerless blank document is supported.
Empty input, including a title-only document after stripping, leaves the body
untouched. `strip-first-h1` removes only the first parsed Markdown H1 (ATX or
Setext), including its heading markup. Fenced code samples are not headings;
all other supplied Markdown remains literal. The existing file's encoding, line
endings, and final-newline policy still apply. A blank line separates the header
from seeded Markdown. Explicit identity and SHA-256 checks also apply to body fills.

Frontmatter results are `path`, `exists`, `changed`, `sha256`, and `body-empty`.
File safety, size, encoding, permission, and no-op rules above apply.
The operation observes the current hash
and uses a guarded atomic write internally; optional `expected-sha256` adds a
caller-supplied precondition. A stale guard or a concurrent edit refuses the write.
Edits are atomic per file, not a transaction across loop iterations.

See the [GitHub project-page example](../examples/github-projects/README.md) for
the complete Eleventy content workflow with plain repository-name filenames and
separate editorial and refreshed metadata.
