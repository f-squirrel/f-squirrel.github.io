---
title: Let it explode!
published: true
permalink: "/let-it-explode/"
tags: [cpp, exceptions, gdb, debugging]
readtime: true
comments: false
---

Exceptions are an inherent part of modern C++.
Everything is clear with the exceptions that can be handled, but what do we do with the exceptions that have no foreseeable resolution?

One of the ways to work with them is to catch, log, and exit the application. Let’s take a look at the following example:

```cpp
#include <iostream>
#include <stdexcept>

void do_something() {
  throw std::runtime_error{"Fatal error occured!"};
}

// Usually this kind of functions runs in a separate thread.
// For the sake of simplicity, I omit the threading part.
void main_loop_function() {
  try {
    while(true) {
      do_something();
    }
  } catch (const std::exception& e) {
    // We assume that at this point the state of the application
    // is unknown and we need to stop it.
    std::cerr << "FATAL ERROR: " << e.what() << std::endl;
    std::exit(1);
  }
}

int main(int argc, char* argv[]) {
  main_loop_function();
  return 0;
}
```

The output and exit code are the following:

```sh
$ ./a.out ; echo $?
FATAL ERROR: Fatal error occured!
1
```

There is an error message and an exit code of 1: since the code is not 0, we know that the process has finished with an error.
The problem faced by a developer is that there is no possibility of checking the stack trace.
Moreover, the exit code looks like the exit was expected, which is not the case!
It seems that we have come across an exception that we are not able to handle.

How can we improve the situation? After logging the exception, we can replace `std::exit(1)` with `std::abort()`:

```plain
FATAL ERROR: Fatal error occured!
[1]    41070 abort (core dumped)  ./a.out
134
```

What is the motive?
First of all, the exit code of the process is `134`(`128 + SIGABRT(6)`) - it has therefore become clear that the application terminated prematurely.

Second of all, a core dump file has been generated, enabling backtrace visibility.
The GDB output is as follows:

```plain
(gdb) bt
#0  __GI_raise (sig=sig@entry=6) at ../sysdeps/unix/sysv/linux/raise.c:51
#1  0x00007f13ab25c801 in __GI_abort () at abort.c:79
#2  0x000055b259c04db8 in main_loop_function() ()
#3  0x000055b259c04ddf in main ()
```

What else can be done?

`std:: terminate` can be used instead of `std::abort`.
The difference is that `std::abort` causes abnormal program termination unless `SIGABRT` is caught by a signal handler,
while `terminate` calls [terminate_handler](https://en.cppreference.com/w/cpp/error/terminate_handler){:target="_blank"}, which, by default, calls `std::abort`.
Let us see the output of the program via `terminate`:

```plain
FATAL ERROR: Fatal error occured!
terminate called after throwing an instance of 'std::runtime_error'
  what():  Fatal error occured!
[1]    41191 abort (core dumped)  ./a.out
134
```

The result is identical, where the exit code is the same and a core dump is generated.
As an occasional bonus, the type and message of the exception are printed (this may not be guaranteed by the standard).

The backtrace is similar, with the exception of two additional function calls to `libstdc++` between `terminate` and `abort`:

<pre>
(gdb) bt
# 0  __GI_raise (sig=sig@entry=6) at ../sysdeps/unix/sysv/linux/raise.c:51
# 1  0x00007f19756aa801 in__GI_abort () at abort.c:79
<span style="background-color: #FFFF00">#2  0x00007f1975cff957 in ?? () from /usr/lib/x86_64-linux-gnu/libstdc++.so.6
# 3  0x00007f1975d05ae6 in ?? () from /usr/lib/x86_64-linux-gnu/libstdc++.so.6</span>
# 4  0x00007f1975d05b21 in std::terminate() () from /usr/lib/x86_64-linux-gnu/libstdc++.so.6
# 5  0x0000556deb28cdb8 in main_loop_function() ()
# 6  0x0000556deb28cddf in main ()
</pre>

But wait, there’s more!

The preferred methodology of this article is to let the exception remain uncaught and leave it to the top-level code to handle it.
In this case, it is the C++ standard library:

```cpp
void main_loop_function() {
  while (true) {
    do_something();
  }
}
```

The output:

```plain
terminate called after throwing an instance of 'std::runtime_error'
  what():  Fatal error occured!
[1]    41273 abort (core dumped)  ./a.out
134
```

The GDB output is the following:

<pre>
(gdb) bt
# 0  __GI_raise (sig=sig@entry=6) at ../sysdeps/unix/sysv/linux/raise.c:51
# 1  0x00007fb2b8009801 in__GI_abort () at abort.c:79
# 2  0x00007fb2b865e957 in ?? () from /usr/lib/x86_64-linux-gnu/libstdc++.so.6
# 3  0x00007fb2b8664ae6 in ?? () from /usr/lib/x86_64-linux-gnu/libstdc++.so.6
# 4  0x00007fb2b8664b21 in std::terminate() () from /usr/lib/x86_64-linux-gnu/libstdc++.so.6
# 5  0x00007fb2b8664d54 in __cxa_throw () from /usr/lib/x86_64-linux-gnu/libstdc++.so.6
<span style="background-color: #FFFF00">#6  0x000055e15c3b4a76 in do_something() ()</span>
# 7  0x000055e15c3b4a95 in main_loop_function() ()
# 8  0x000055e15c3b4aab in main ()
</pre>

Now there is no log message, but the output clearly defines the function that threw the exception, which is the essence of this whole endeavor.

So let it explode and fear not of core dumps!

Note:
Reliance on core dumps is a viable option only if the system is configured to generate them and they are accessible.

Please share your thoughts on [LinkedIn](https://www.linkedin.com/posts/ddanilov_cpp-cplusplus-activity-6673490005106806784-GP9J?utm_source=share&utm_medium=member_desktop).
