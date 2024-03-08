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

In 2020 we have started adopting Rust in our company. This post is my self-reflection as a software engineer and a manager of a team using Rust for commercial purposes.

## Background

We are a strong backend team with extensive experience in utilizing modern C++ features: heavy template usage, constexpr, enable_if, variants, lambdas, what not.

We had to build a hive of services responsible for scanning various Web3 financial data, processing, normalizing it and storing the results in a database, together with providing an HTTP api for the data retrieval.

It was crucial for us to be able to serve a high number of users simultaneously. Since the data is financial, the cost of an error is high. This led to a decision that data structures had to be strictly typed. We had an experience of utilizing such structures in the C++ part of the projects.

## Reasoning behind choosing Rust