---
title: Five ways to harden C/C++ project
published: true
permalink: "/ways-to-harden-c-cpp-project/"
tags: [c, cpp, debugging]
readtime: true
---

C and C++ are powerful languages allowing the creation of complex and
performance-sensitive applications. However, with great power comes great
responsibility: C and C++ compilers assume that developers know what they are
doing. It means that code containing undefined behavior, non-standard language
extensions, and dangerous casting is considered legitimate. I have been writing
about subtle nuances of the languages, problems that they introduce, and how to
avoid them. In this post, I will share five efficient ways to identify
dangerous pieces of code at a very early stage; I will start with diagnostics
that are relatively easy to implement and get to more complicated ones.

## #1 Restrict compilation flags

In one of my [previous posts](/default-non-standard-features/), I have
described how the restriction of compilation flags in C++ helps to prevent a
segmentation fault triggered by inaccurate usage of the C99 feature
"variable-length array". Unfortunately, this is just one of many examples of
how mixing up language extensions and features from C and C++ standards can
complicate development. I would recommend the following three flags supported
by both GCC and Clang that will help to minimize the risk of encountering these
problems.

* `-pedantic` Issue all the warnings demanded by strict ISO C and ISO C++;
reject all programs that use forbidden extensions and some other programs that
do not follow ISO C and ISO C++.
* `-Wall` This enables all the warnings about constructions that some users
consider questionable, and that is easy to avoid (or modify to prevent the
warning), even in conjunction with macros.
* `-Werror` Make all warnings into errors. Unfortunately, when a deadline is
close, developers tend to ignore warnings. To make sure that no potential
problem is missed, it is strongly recommended to treat warnings as errors.

Since I work on Linux and macOS platforms, I have less experience with the
Microsoft Visual C++ compiler. I have done short research about MSVC analogs of
the flags mentioned above and according to StackOverflow the closest options
are:

* `/W4` is the highest level of warnings
* `/WX` is to treat warnings as errors

Note that after implementing the restrictions, there still may be places using
platform-specific features that are not compatible with the flags recommended
above. The best way to handle it is to isolate platform-dependent code in
separate files and instruct the compiler to suppress the errors for these
files. For information on how to do it, please refer to the compiler's
documentation.

## #2 Use several compilers

C and C++ are in a unique position where there is a standard defined by the C
or C++ committee and actual implementations created by different teams: Clang,
GCC, MSVC, Intel, etc. In an earlier post [“Implementations of std::async
and how they might Affect Applications”](/std-async-implementations/), I
have demonstrated how differently compiler teams may read the C++ standard,
which leads to different implementations of the very same feature.
Compilers not only may have different implementations of language features but
also different diagnostics of the code. For cross-platform projects, it is
often mandatory to be compatible with a few compilers, but even a
single-platform code can benefit from being checked by several compilers.<br>
For example, on Linux and macOS, Clang and GCC can be easily used for this
purpose because both support the same flags out of the box.<br>
On Windows, the Intel C++ Compiler can be used as an addition to Microsoft
Visual C++ Compiler, because Intel [supports
MSVC's](https://software.intel.com/content/www/us/en/develop/documentation/oneapi-dpcpp-cpp-compiler-dev-guide-and-reference/top/compatibility-and-portability/microsoft-compatibility.html)
compilation flags. Also, Clang is [partially
compatible](https://clang.llvm.org/docs/MSVCCompatibility.html) with MSVC,
which may be good enough for many projects.

## #3 Static analysis

Despite modern compilers have done a good job in improving diagnostics, they
have to stay conservative to not scare users off with tons of new error/warning
messages. In this case, a static analyzer comes to help. The static analyzer
performs the analysis of code without actually running it. There are many
analyzers on the market, but I’d like to recommend **Clang-Tidy**. It is a
Clang-based C/C++ “linter” tool. Its purpose is to provide an extensible
framework for diagnosing and fixing typical programming errors, like style
violations, interface misuse, or bugs that can be deduced via static analysis.
It provides various groups of checks improving application's performance and
avoiding potential bugs. For example, `bugprone-use-after-move` warns a user
when an object is used after “move”. From the C++ standard’s perspective, it is
a legitimate behavior but in real life, it may lead to unexpected problems when
a developer is not aware of the actual state of the object after the move.
You may find an example in the post ["The moving truth behind
std::optional"](/the-state-of-std-optional-after-move/).<br> Clang-Tidy has a
very intuitive configuration, amazing documentation, and several major
categories of checks:

* `bugprone` -  avoid typical programming errors leading to bugs.
* `cppcoreguidelines` - ensures that the code is compliant with [C++ Core
Guidelines](https://github.com/isocpp/CppCoreGuidelines).
* `modernize` - C++ is moving forward, and it is easy to miss new features;
this check identifies places where the code may be improved or shortened by
using them.
* `performance` - since C and C++ are about performance, it would be sad to
miss a copy of a long string instead of passing by reference or moving.
* `readability` - everybody knows that a code written once but is read a
hundred times, so it is really good to invest in readability.

Clang-Tidy has many more checks which can be found on the
[official page](https://clang.llvm.org/extra/clang-tidy/checks/list.html).

## #4 Runtime memory analysis

C and C++ are famous for the freedom of memory management that they provide but
also for problems that this freedom may lead to: memory leaks, double-free,
usage of uninitialized variables, etc. In order to catch this kind of
error, a developer has to use runtime analysis. A runtime analyzer
requires a program to be executed so the analyzer can actually detect
problematic memory usage and report it. For many years Valgrind was the
leader of memory profiling but in this article, I’d like to talk about the
tools provided by Clang. In the Clang project, they are called sanitizers
and require the application to be compiled and linked with an additional
flag `fsanitize=<sanitizer name>`. Most of the sanitizers have good
documentation so I am going to name only the most useful onces.

[**AddressSanitizer**](https://clang.llvm.org/docs/AddressSanitizer.html) is a
fast memory error detector. It consists of a compiler instrumentation module
and a runtime library. ASan can detect the following types of bugs:

* Out-of-bounds accesses to heap, stack and globals
* Use-after-free
* Use-after-return
* Use-after-scope
* Double-free, invalid free
* Memory leaks (experimental)

Typical slowdown introduced by AddressSanitizer is 2x.<br>

[**LeakSanitizer**](https://clang.llvm.org/docs/LeakSanitizer.html) is a
run-time memory leak detector. It can be combined with AddressSanitizer to get
both memory error and leak detection, or used in a stand-alone mode. LSan adds
almost no performance overhead until the very end of the process, at which
point there is an extra leak detection phase.

[**MemorySanitizer**](https://clang.llvm.org/docs/MemorySanitizer.html) is a
detector of uninitialized reads. MSan consists of a compiler instrumentation
module and a runtime library.
Typical slowdown introduced by MemorySanitizer is 3x.

## #5 Runtime thread safety analysis

When it comes to multithreading, it is a world of mysterious timing issues; an
application may run fine for months until a variable not protected with a mutex
will lead to a data race. Neither unit tests nor end-to-end tests provide a
full guarantee of thread safety. Fortunately, Clang Tools have
[**ThreadSanitizer**](https://clang.llvm.org/docs/ThreadSanitizer.html).
ThreadSanitizer is a tool that detects data races. It consists of a compiler
instrumentation module and a run-time library. Typical slowdown introduced by
ThreadSanitizer is about 5x-15x and typical memory overhead introduced by
ThreadSanitizer is about 5x-10x. Need to mention that ThreadSanitizer is in
beta, however many prominent projects use it actively.


It is important to mention that runtime analyzers have a
limitation: they can analyze only the code that is being executed, i.e. if some
rare case if-branch has a bug but not executed while running the program built
with the analyzer, the runtime analyzer will not show the problem. The takeaway
here is: improve your unit and end-to-end tests to cover as much as possible!
