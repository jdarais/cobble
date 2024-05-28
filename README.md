# Cobble
Cobble is a multi-project, multi-environment build automation tool

Cobble allows you to define projects and tasks in your repository, which are contained in a single Cobble "workspace".  Tasks can depend on assets and tasks from other projects in the workspace, allowing for the creation of a full-workspace task dependency graph.  Cobble can be compared to build automation tools like make or doit, but with a focus on on ease of use and multi-project support.

Cobble is:

- __Fast__: Cobble is built with technologies that are well-suited for writing fast native applications, including Rust and Lua
- __Multi-platform__: Cobble runs on Linux, Mac, and Windows
- __Small__: Download size <5MB
- __Easy to Install__: Download is a single executable binary
- __Easy to Use__: Cobble uses a simple configuration language and CLI interface to automate build tasks

Cobble is not:

- A build tool: Cobble itself doesn't know how to build anything; but it should be able to integrate with just about any build tool.
- A batteries-included build environment: Cobble does not provide set-up or boilerplate configuration for any specific language platforms. Its aim is to make it as easy as possible to do that yourself.

## Why Cobble?

Most single-project software repositories use tooling that provides capabilities like project configuration, package management, environment isolation, and package build/publish, (like cargo, npm, poetry, go, etc.)  These work very well for single-application or single-library projects, but automating build and publish tasks in a multi-project repository, (i.e. a monorepo,) is beyond the scope of these tools.  For multi-project repositories, developers often turn to monorepo tools like Bazel, Pants, or Nx, however these monoroepo tools come with their own challenges.  Many,-- including their plugin ecosystems-- are centered around a small number of language platforms.  Additionally, any integration with a language platform's core toolset or an IDE must be built for that monorepo tool.  In adopting one of these monorepo tools, you become reliant on its ecosystem being healthy and aligned enough with your needs to provide the language and IDE integration support that you would otherwise get with individual, single-project repositories using the core tooling for that platform.

Cobble aims to provide an alternative to monorepo build tools, allowing repositories to combine multiple projects without giving up the strong tools and integrations ecosystems that have built up around the individual project platforms, while supporting a broad set of tools and language platforms, and keeping the interface and configuration language as simple as possible.


