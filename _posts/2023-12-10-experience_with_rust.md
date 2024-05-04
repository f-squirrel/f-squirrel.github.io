---
title: Experience with Rust
published: true
permalink: "/experience-with-rust"
tags: [rust]
readtime: true
comments: false
---

I learned about the Rust project in 2012 or 2013 and straight away liked the language, because it had everything I wanted it to be: it was compiled, strictly typed, explicit, supported templates and had no garbage collector. However, as it often happens I was only observing the project but never used it in production.

In 2015 the first Rust version was released officially and the whole ecosystem started blooming.

In 2020 we started adopting Rust in our company. This post is my self-reflection as a software engineer and a manager of a team using Rust for commercial purposes.

## Background

We are a strong backend team with extensive experience in utilizing modern C++ features: heavy template usage, `constexpr`, `enable_if`, variants, lambdas, and whatnot.

We had to build a hive of services responsible for scanning various Web3 financial data, processing, normalizing it and storing the results in a database, together with providing an HTTP API for the data retrieval.

We needed to be able to serve a high number of users simultaneously. Since the data is financial, the cost of an error is high. This led to a decision that data structures had to be strictly typed. We had an experience of utilizing such structures in the C++ part of the projects.

## Reasoning behind choosing Rust

When you have to develop a service in 2020, you usually do not go for C++. It has no standard package manager or a build system, C++ is not aware of networking on the level of the standard library, and the existing asynchronous solutions (boost, `libev`) are hard to handle and often introduce lots of their own primitives.

Other options were Python and Typescript. We have wide experience in Python, however, it is not trivial to make it asynchronous, its type hints are not useful because they are not an integral part of the language and are validated by 3rd party tools like Pyright.
Additionally, its exposure to Web3 is very limited.

Typescript was very promising: it is kind of strictly typed, compiled to JavaScript and widely used in Web3. However, it is still kind of strictly typed and allows using Javascript libraries, which are not strictly typed. This is a big problem for us because we wanted to have a strictly typed system.

* Ecosystem
  * Serde is awesome
  * Crypto crates
  * Tokio + Web frameworks
  * Crates: a single site (and lib.rs), lots of libraries (I might be biased; at least there are a lot of them actual for my field)
* Incredible tools
  * Rust analyzer
  * Audit
  * Cargo
  * Clippy
  * Xtask
  * Vscode, CLion, new Jerbrains IDE
* Explicitly (unwrap, result, option, types, self, etc.)
* Configurability
* Templates like Concept in C++
* Weird turbo fish operator. I know it is for easier parsing, but I would prefer to have it as [] or as in C++.
* C++ developers improving
* C++ templates are easier to use
* Compilation errors (thanks to Tsvetomir)
  * Seasoned developers go nuts
  * I have no idea how people with only dynamic language experience manage to use it
* Not very good for prototyping
* Very bad for refactoring
* Compilation time, no incremental build

## Ecosystem

The Rust ecosystem stands out as one of its most compelling features, offering a rich assortment of tools, libraries, and resources that empower developers to build robust and efficient applications. At the heart of this ecosystem are **crates**, Rust's package format, which serve as the fundamental building blocks of Rust projects. Crates encapsulate functionality, promoting modularity, code reuse, and clean architecture.

Central to the Rust ecosystem is **Crates.io**, the primary repository for Rust crates. Crates.io acts as a centralized hub, akin to npm for JavaScript or PyPI for Python, where developers can discover, share, and distribute crates. With thousands of crates covering a wide range of domains and use cases, Crates.io provides developers with a wealth of options to accelerate their development process.

Among the standout libraries on Crates.io are **Serde**, **log**, and **tracing**, each renowned for its versatility, extensibility, and robustness. Serde, for instance, is a powerful serialization framework that supports seamless data interchange between Rust data structures and various formats such as JSON, YAML, and bincode. Similarly, log and tracing offer comprehensive logging and diagnostic capabilities, empowering developers to instrument their applications with ease.

What sets these libraries apart is their extensibility and adaptability. With a multitude of implementations available, developers can seamlessly swap out implementations without necessitating changes to their codebase. This flexibility not only simplifies maintenance but also future-proofs Rust applications, ensuring they remain resilient to evolving requirements and technologies.

## Tools for Rust Development

The Rust ecosystem is full of tools that are meant to increase efficiency, optimize processes, and guarantee the quality of the code. Let us examine some of the exceptional resources that have grown to be essential for Rust programmers:

### Rust Analyzer: Intelligent IDE Support

Rust Analyzer stands out as a game-changer in the Rust development landscape. As an advanced language server, Rust Analyzer offers comprehensive IDE support, including code completion, syntax highlighting, and error checking. Its lightning-fast analysis engine provides real-time feedback, enabling developers to write code with confidence and efficiency.

### Cargo: The Swiss Army Knife of Rust

Special recognition should be given to Cargo, the package manager and build tool for Rust, for its strength and versatility. With just one command, developers can build projects, run tests, and manage dependencies with ease using Cargo. It is a key component of Rust development because of its user-friendly CLI interface and strong dependency resolution mechanism.

### Audit: Security and Dependency Analysis

In today's software development world, security is critical, and Rust's Audit tool assists developers in maintaining the highest standards. Developers can proactively discover and reduce security concerns by using Audit to analyze project dependencies for known vulnerabilities, thereby safeguarding the integrity and safety of Rust programs.

### Xtask: Extensible Task Runner

Xtask simplifies the automation of common development tasks by providing a flexible and extensible task runner for Rust projects. With Xtask, developers can define custom build scripts, automate testing, and orchestrate complex workflows—all within the familiar Rust ecosystem.

### IDE Support: Seamless Integration with Popular Editors

Rust boasts robust support for popular integrated development environments (IDEs) such as Visual Studio Code (VS Code), CLion, and JetBrains' new Rust-centric IDE. These IDEs offer rich features, including code navigation, debugging, and refactoring tools, tailored specifically for Rust development. With seamless integration into the Rust ecosystem, developers can leverage the full power of their preferred IDE while coding in Rust.
