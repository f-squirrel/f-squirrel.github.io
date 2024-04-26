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
