# GitHub project pages

This example uses ordinary `.task` capabilities to create Markdown documents for
an Eleventy website. No GitHub-specific command, shell helper, .NET SDK, or Rust
task compiler is involved. The native CLI performs the requests and file edits.

From the dotask source checkout on Windows:

```powershell
cd C:\Users\kodyb\Projects\dotask
.\build.cmd --use-dir examples/github-projects/.tasks help refresh-projects
.\build.cmd --use-dir examples/github-projects/.tasks refresh-projects
```

On Linux/macOS, replace `.\build.cmd` with `./build.sh`. Help performs no requests
and creates no project pages. The refresh command contacts GitHub and writes only
under this example's ignored `src/projects/` directory by default. Change the
account with `--api-url https://api.github.com/users/USERNAME/repos`, and choose a
different content directory with `--output PATH`.

For an existing website, copy [.tasks/refresh-projects.task](.tasks/refresh-projects.task)
into its `.tasks` directory, adjust the endpoint/output defaults, and run
`dotask refresh-projects`. The website must have its own root `.dotasks.yaml`.

The task fetches all pages before changing documents, excludes forks, and creates
`src/projects/fixeol.md` for a repository named `fixeol`. It initializes `title`,
`summary`, `screenshot`, and `hidden` only on creation. Every refresh updates only
the eight declared `github` fields. Write your own summary, screenshots, and notes
in the generated file: subsequent refreshes preserve them. The site's templates
should use a nonblank `summary` when present and otherwise `github.description`.
That display fallback belongs to the website; dotask does not render the site.

Existing files retain comments, body text, encoding, line endings, and final
newline state. Repeating an unchanged refresh leaves bytes and modification times
unchanged. A mismatched `github.id` stops before changing that document. Rename a
page manually before refreshing a renamed repository; this example does not scan
for old names, rename files, or delete pages absent from the API response.

Requests use no token by default and may encounter GitHub's public API limits.
For authenticated use, explicitly configure an `Authorization` header using
`{environment: GITHUB_AUTHORIZATION}` containing the complete header value.
Do not put credentials in URLs or source files. HTTP failures publish no partial
collection. File updates are atomic per document; a later failure does not undo
documents already updated during that refresh.

See [native YAML workflows](../../docs/YAML-WORKFLOWS.md) for pagination limits,
result scopes, JSON operations, and supported frontmatter edits.
