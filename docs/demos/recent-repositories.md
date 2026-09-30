# Recent repositories demonstration

[Watch the demonstration](recent-repositories.mp4).

The recording shows the embedded web interface from the rebuilt feature
implementation (`e42fdb8`). It uses local fixtures and mocked successful scan
responses, with an isolated history directory and no GitHub credentials or
live GitHub access. Repository listing and removal use the running server's
actual API and persistent storage.

The demo shows two scans listed newest first, persistence after restarting the
server and reloading the page, selection of a saved repository, a repeat scan
updating its existing entry, removal while keeping current results, and
persistent removal after another restart. The same run verifies that the CLI
reads the shared list.

The captions describe the demonstrated actions; they are video annotations,
not application UI. The video is stored in this repository so reviewers can
access it with their existing repository permissions.
