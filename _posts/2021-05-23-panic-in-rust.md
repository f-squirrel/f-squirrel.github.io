---
title: Panicking in Rust
published: true
share-description: ""
tags: [rust, rustlang, panic]
share-img: /img/1200px-Rust_programming_language_black_logo.svg.png
readtime: true
permalink: "/panic-in-rust"
share-description: "The difference between abort and panic"
---

The Rust programming language provides a way to terminate a program when it reaches an unrecoverable state by calling
the macro `std::panic!`. I personally like this reference to kernel panic. It comes in handy when a developer needs to
assert in example code or unit tests and it is eventually called by the method `unwrap` of `Option` and `Result` enums.

For me, as a C/C++ engineer (I hope C and C++ people will forgive me the blasphemy of putting slash between the two
        languages), `panic!` was initially a synonym of `abort` in C or `terminate` in C++ but with nice features like
stack unwinding. However, the practice revealed some differences which I am going to go through in this post.

Let us start with a simple program that panic immediately after the start.
```rust
fn main() {
    println!("Hello, world!");
}
```

The program is terminated and the output shows the place triggering the panic. As a bonus, the application can be
configured via an environment variable to show a backtrace (stack unwinding).
```plain
$ cargo run
Hello, world!
thread 'main' panicked at 'Panic in the main thread!', src/main.rs:19:5
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

The interesting part starts when the exit code of the process is checked right after the termination:
```plain
$ echo $?
101
```

Rust sets the exit code to `101` explicitly when a process panics by calling the `exit` function while `abort` signals
the kernel to kill the process (the detailed explanation of how `abort` works on Unix system can be found in an earlier
        [post](/how-signals-are-handled-in-a-docker-container)). In practice, it means that no core dumps will be
generated in the default configuration*.

Now, let us take a look at what happens when `panic!` is called from a non-main thread.
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
The output clearly says that the thread has panicked but the main thread continues running even after calling `join`! It
leads to a conclusion that `panic` exits only the current thread rather than the whole process which is completely
different from C’s `abort`!

Rust would not be Rust if it did not provide elegant ways to terminate the program in the case when a background thread
crashes.

Obviously, the language provides elegant ways to terminate the program in the case when one of the background threads
crashes.

## Unwrap the result of join
The shortest way is to `unwrap` the return value of `join`:
```rust
...
handle.join().unwrap();
...
```
The method `join` returns a result with error, unwrapping leads to panic, however this time in the main thread:
```plain
$ cargo run
Thread started!
thread '<unnamed>' panicked at 'Panic in a thread!', src/main.rs:7:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Any', src/main.rs:10:19
```

## Check or match the result of join
Another option is to check the result of join and decide what to do in runtime:
```rust
match handle.join() {
    Ok(_) => println!("Joined!"),
    Err(_) => println!("Join failed"),
};
```
Note that this example ignores the error and the program exits with the code `0`.


## Configure the panic! to call the abort function
Yes, Rust provides this possibility as well! It can be done via Cargo.toml file of the project:
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

## Conclusion
I see this as another example of Rust’s amazing flexibility which a developer should be aware of.

