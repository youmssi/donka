# Studio API reference

Every Studio installation serves an interactive Scalar reference at `/api/docs`. It uses that
installation's own OpenAPI document, available at `/api/v1/openapi.json`, so the displayed
version and operations match the running Studio image. The generated reference assets are
self-hosted with the Studio web export; loading the page does not require a third-party CDN.

The documentation site publishes a static reference for each curated Studio, Runtime and CLI
release set. Use `/api/docs` when you need to inspect the API of a particular installation.
