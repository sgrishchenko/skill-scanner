# Local web server

Status: implemented. Provides a local browser interface from the same
executable as the [CLI](cli-report.md).

## Launch and delivery

Run `skill-scanner serve [--port PORT]`:

```console
skill-scanner serve
skill-scanner serve --port 8080
```

Bind only to IPv4 loopback. The default URL is `http://127.0.0.1:3000`;
`--port 0` selects an available port. Print the actual URL to stderr. Non-numeric
ports and values outside 0–65535 are usage errors (exit 2). An occupied port or
startup failure produces a diagnostic and exit 1.

Users open the printed URL in a current browser with JavaScript enabled.
Ctrl+C stops the process. Do not launch a browser automatically.

Embed HTML, CSS, JavaScript, and an SVG favicon in the executable so the site
works from any working directory. Source builds use Cargo; no asset folder,
Node.js runtime, frontend build, CDN, external fonts, images, analytics, or
frontend network dependencies are required. Existing `scan` commands retain
their output and exit-code contract. Hosted deployment, public binding, and
hosted authentication remain outside [product scope](../README.md#product-boundaries).

## Local request protection

- Validate Host against `127.0.0.1:PORT` or `localhost:PORT`; the port can be
  omitted for HTTP port 80.
- Validate any Origin against the exact HTTP origin for that Host.
- Require `X-Skill-Scanner: 1` and JSON for POST requests. Limit request bodies
  to 4 KiB. Grant no CORS access.
- Keep the optional process-level `GITHUB_TOKEN` in the server environment/client
  under the [repository access](repository-access.md#github-access) rules.

These checks prevent other websites from using the local process's GitHub
credentials through browser requests or DNS rebinding. Routes and rejection
statuses are defined in the [web scan API](web-scan-api.md).

## Browser content protection

Serve a restrictive Content Security Policy: scripts, styles, images, and
fetches are same-origin only, and framing is disabled. Add `nosniff`,
`no-referrer`, and `no-store`.

Render repository text with DOM text nodes, without HTML or Markdown evaluation.
Source links must use the HTTPS GitHub origin and open in a new tab with
`noopener noreferrer`. The interface explains that discovery does not certify
safety or compatibility, that it never runs skills, and that skills are
installed only on request, as described in [Codex skills](codex-skills.md).

See [web scan](web-scan.md) for concurrency and lifecycle, and
[web results](web-results.md#accessibility-and-layout) for accessibility.

## Acceptance checks

- The installed binary serves the interface from any working directory.
- Default, custom, available (`0`), invalid, and occupied ports behave as
  specified.
- Foreign origins/hosts, unmarked requests, and oversized bodies are rejected.
- Repository-controlled markup remains inert text and source links use the
  required origin and opener protections.
- Browser responses never expose GitHub credentials.
- Existing CLI scan behavior and tests continue to pass.
