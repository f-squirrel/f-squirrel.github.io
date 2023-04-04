---
title: How to overwrite a file in Rust
published: true
share-description: "Nuances of File usage in Rust"
tags: [rust, rustlang, filesystem, file]
share-img: /img/1200px-Rust_programming_language_black_logo.svg.png
readtime: true
permalink: "/how-to-overwrite-a-file-in-rust"
share-description: "Nuances of writing to files in Rust"
comments: false
---

Recently, I took an interest in the Rust programming language due to its performance and safety.<br>
I chose Rust because of its RAII-based memory model, high-level abstractions which do not compromise perfomance, and the ability to disable its standard library and develop for bare metal. As a bonus, it comes with incredible static checks, dependency management tools, as well as unit test and benchmark frameworks.<br>
The language itself is relatively new and not as popular as other system programming languages, though major enterprises have used it in key products, such as Facebook Diem, AWS Firecracker and Dropbox.

While I was playing around with its capabilities, I attempted to overwrite an existing text file. Naturally, I wrote the following piece of code to do the trick:

```rust
use std::io::Write;

fn main() -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new().write(true).open("./file")?;
    f.write_all(b"XXX")?;
    f.flush()?;
    Ok(())
}
```

The original content of the file was `AAAAAAAA` and after execution of the
program, it unexpectedly became `XXXAAAAA` rather than just `XXX`. I was quite surprised because in most of the languages that I am familiar with, when a file is opened with the write (`"w"`) flag, the existing content would be automatically truncated and overwritten. Apparently, to achieve the same in
Rust, `truncate(true)` needs to be added:

```rust
std::fs::OpenOptions::new().write(true).truncate(true).open("./file")?;
```

I like the granularity of file settings provided by Rust via low-level flags, such as `O_WRONLY` and `O_TRUNC`, but beginners need to be aware of them.


Please share your thoughts on [Twitter](https://twitter.com/dbdanilov/status/1371754759720435715?s=20).