---
title: Terminating with panic! in Rust
published: true
share-description: ""
tags: [rust, rustlang, panic, abort, coredump]
share-img: /img/rust.png
readtime: true
permalink: "/panic-in-rust"
share-description: "Various process terminating methodologies using panic! in Rust"
comments: false
---

The Rust programming language provides a few ways to terminate a program when it reaches an unrecoverable state by calling
the macro `std::panic!` - a reference to kernel panic that I have found quite amusing.<br>
It comes in handy when an assert needs to used within code, such as for unit tests, and it is eventually called by the
method `unwrap` of the `Option` and `Result` enums.

From my experience as a C/C++ engineer (I hope C and C++ enthusiasts, as well as the almighty coding standard Gods, will
        forgive me for this blasphemy of placing a slash between the two languages), `panic!` was initially a synonym of
`abort` in C and C++, but with a few more features, such as stack unwinding. The goal of this post is to shed some light
on a few of the differences between `panic!` and `abort` that I have personally encountered.

Let us start with a simple program that immediately 'panics' when it is run:

```rust
fn main() {
    panic!("Panic in the main thread!");
    println!("Hello, world!");
}
```

The program is terminated and the output reveals where the panic was triggered. As a bonus, the application can be
configured via an environment variable to show its backtrace (stack unwinding).

```plain
$ cargo run
Hello, world!
thread 'main' panicked at 'Panic in the main thread!', src/main.rs:2:5
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

It becomes even more intriguing when the exit code of the process is checked right after termination:

```plain
$ echo $?
101
```

Rust sets the exit code to `101` explicitly when a process panics by calling the `exit` function, while `abort` signals
the kernel to kill the process (a detailed explanation of how `abort` works on Unix systems can be found in an earlier
        [post](/how-signals-are-handled-in-a-docker-container){:target="_blank"}). In practice, this means that no core dumps are
generated in the default configuration.

Now, let us take a look at what happens when `panic!` is called from a sub-thread:

```rust
use std::thread;

fn main() {

    let handle = thread::spawn( || {
        println!("Thread started!");
        panic!("Panic in a thread!");
    });

    handle.join();

    println!("Hello, world!");
}
```

Output:

```plain
$ cargo run
Thread started!
thread '<unnamed>' panicked at 'Panic in a thread!', src/main.rs:7:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
Hello, world!

$ echo $?
0
```

The output clearly states that the thread has panicked but the main thread continues running, even after calling `join`!
It can thus be concluded that `panic` does not exit the entire process, but rather only the current thread; this is
completely different from C’s `abort`!

My continued interest in the Rust language grows precisely due to features such as this, where the language provides
elegant methods for terminating a process in the case where a background thread crashes.

If we were to force an ultimatum on the result of `join`, the shortest way is to `unwrap` the return value:

```rust
...
handle.join().unwrap();
...
```

The result contains an error and unwrapping leads to panic in the main thread:

```plain
$ cargo run
Thread started!
thread '<unnamed>' panicked at 'Panic in a thread!', src/main.rs:7:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Any', src/main.rs:10:19
```

Another way to manipulate the output of `join` is to check the result and decide what to do during runtime; the
following example uses `match`:

```rust
match handle.join() {
    Ok(_) => println!("Joined!"),
    Err(_) => println!("Join failed"),
};
```

Note that this example only prints the error and the program still exits with `0`.

But wait, there's more!
For those who are not big fans of change, Rust even provides the possibility to configure `panic!` to call `abort`; this
can be done via Cargo.toml in the project:

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

The result is the same as calling `abort` in C: the application is terminated with `SIGABRT` and if the system is
configured, a core dump is generated:

```plain
$ cargo run
Thread started!
thread '<unnamed>' panicked at 'Panic in a thread!', src/main.rs:7:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
[1]    67943 abort      cargo run
134
```

Rust's flexibility truly does not cease to amaze and I will diligently continue to provide such examples which I believe
other enthusiasts should be aware of and use.

*Special thanks to [Rina Volovich](https://www.linkedin.com/in/rina-volovich/){:target="_blank"} for editing.*

Please share your thoughts on [Twitter](https://twitter.com/dbdanilov/status/1399435722441084931?s=20), or [LinkedIn](https://www.linkedin.com/posts/ddanilov_terminating-with-panic-in-rust-dmitry-activity-6805200803901530112-zQGX?utm_source=share&utm_medium=member_desktop).
