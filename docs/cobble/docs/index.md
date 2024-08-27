# About Cobble

## What is Cobble?

Cobble is a multi-project, multi-environment build automation tool.

Cobble allows you to define tasks across a collection of projects, which are contained in a single Cobble "workspace".  Tasks can depend on assets and tasks from other projects in the workspace, allowing for the creation of a full-workspace task dependency graph.  When executing any task, Cobble ensures that all of the task's dependencies have been executed first.

Cobble is:

- __Fast__: Cobble is built with technologies that are well-suited for writing fast native applications, including Rust and Lua, and can run tasks in parallel for fast builds.
- __Cross-platform__: Cobble runs on Linux, Mac, and Windows
- __Small__: Download size is <5MB
- __Easy to Install__: Download is a single executable binary, with no library or script environment dependencies
- __Easy to Use__: Tasks are defined in Lua using a simple interface

## Why Cobble?

The world of software project and repository management seems to be stratified between two extremes.  On one extreme, you may find yourself with a sea of small, single-project repositories based on the favored package management and build stack for that language, such as cargo, npm, yarn, poetry, go, etc.  On the other extreme, you may have a monorepo, with all projects combined into one repository, managed by a complex and restrictive monorepo tool such as bazel, pants, or nx.

Often, the ideal lies somewhere in the middle: clusters of projects grouped into repositories based on which projects make sense to "release" together.  The problem, however, is that this paradigm is not well supported by either the single-repository-focused tools or the heavyweight monorepo tools; or if it is supported, it is only for a narrow set of language platforms.

What I've found to be lacking in our current software development ecosystem is a simple, lightweight, general-purpose build automation tool that has a low barrier to entry and can orchestrate tasks across projects within a repository.  Cobble aims to be a tool that fills that gap.
