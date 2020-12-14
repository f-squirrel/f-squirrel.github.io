---
title: How Clang helps to understand object's memory layout
published: true
permalink: "/memory-layout/"
tags: [C++, clang, clang-tools]
readtime: true
---
In my pet project, I was sending multiple C structs over the network but on the receiver side, they were corrupted.  I
had a hunch the issue was related to the [alignment](https://en.wikipedia.org/wiki/Data_structure_alignment) but all the
structures had `#pragma pack(push, 1)` and `#pragma pack(pop)`.
<br>The picture below represents possible memory layout of a struct containing `char`, `int` and `long`:
<p align="center">
  <img src="/img/mem_layout.svg">
</p>

Since the real structs were defined in separate header files, I put them in a single header for simplicity’ sake.
```cpp
// net_headers.h
#include <cstdint>

#pragma pack(push, 1)
struct Header1 {
    uint8_t msg_type;
};
#pragma pack(pop)

struct Header2 {
#pragma pack(push, 1)
    uint8_t value_1;
    uint64_t value_2;
    uint32_t value_3;
    uint32_t value_4;
    uint32_t value_5;
#pragma pack(pop)
};

#pragma pack(push, 1)
struct Header3 {
    uint8_t msg_type;
    uint32_t value_1;
    uint32_t value_2;
    uint64_t value_3;
    uint64_t value_4;
    uint64_t value_5;
};
#pragma pack(pop)

#pragma pack(push, 1)
struct Header4 {
    uint8_t msg_type;
    uint64_t value_1;
    uint64_t value_2;
    uint64_t value_3;
};
#pragma pack(pop)
```


A careful reader has probably already found the issue but I’d like to show how I discovered it.
I decided to check what memory layout the compiler would create, fortunately, Clang provides this possibility.

First of all, I created a simple compilation unit (cpp file) using the structs:
```cpp
// foo.cpp
#include "net_headers.h"

void foo() {
    Header1 h1;
    Header2 h2;
    Header3 h3;
    Header4 h4;
}
```

After that, I generated a preprocessed version of the compilation unit.
In order to do it, a user needs to add the flag `-E` to a regular compilation command to instruct Clang to run only the preprocessor.

<pre>
$ clang -E \
        -I/home/user/example \
        -std=c++1z \
        foo.cpp > foo_preprocessed.cpp
</pre>

After the preprocessed file is created, I can dump the memory layout.
Note that `-cc1` stands for Clang front-end that actually dumps the layout.
<pre>
$ clang -cc1 -fdump-record-layouts foo_preprocessed.cpp

*** Dumping AST Record Layout
         0 | struct Header1
         0 |   uint8_t msg_type
           | [sizeof=1, dsize=1, align=1,
           |  nvsize=1, nvalign=1]

*** Dumping AST Record Layout
         0 | struct Header2
         0 |   uint8_t value_1
        <span style="background-color: #FFFF00"> 8</span> |   uint64_t value_2
        <span style="background-color: #FFFF00">16</span> |   uint32_t value_3
        <span style="background-color: #FFFF00">20</span> |   uint32_t value_4
        <span style="background-color: #FFFF00">24</span> |   uint32_t value_5
           | [sizeof=32, dsize=32, <span style="background-color: #FFFF00">align=8</span>,
           |  nvsize=32, nvalign=8]

*** Dumping AST Record Layout
         0 | struct Header3
         0 |   uint8_t msg_type
         1 |   uint32_t value_1
         5 |   uint32_t value_2
         9 |   uint64_t value_3
        17 |   uint64_t value_4
        25 |   uint64_t value_5
           | [sizeof=33, dsize=33, align=1,
           |  nvsize=33, nvalign=1]

*** Dumping AST Record Layout
         0 | struct Header4
         0 |   uint8_t msg_type
         1 |   uint64_t value_1
         9 |   uint64_t value_2
        17 |   uint64_t value_3
           | [sizeof=25, dsize=25, align=1,
           |  nvsize=25, nvalign=1]
</pre>
After the layout was printed I’ve noticed that the `struct Header2` had `align=8` while all the rest had `align=1`. That’s right, I’ve placed the pragmas inside the struct:

```cpp
struct Header2 {
#pragma pack(push, 1)
    uint8_t value_1;
    uint64_t value_2;
    uint32_t value_3;
    uint32_t value_4;
    uint32_t value_5;
#pragma pack(pop)
};
```

After putting them outside as for the rest of the structs, the alignment eventually becomes 1.
<pre>
*** Dumping AST Record Layout
         0 | struct Header2
         0 |   uint8_t value_1
        <span style="background-color: #00CC66"> 1</span> |   uint64_t value_2
        <span style="background-color: #00CC66"> 9</span> |   uint32_t value_3
        <span style="background-color: #00CC66">13</span> |   uint32_t value_4
        <span style="background-color: #00CC66">17</span> |   uint32_t value_5
           | [sizeof=21, dsize=21, <span style="background-color: #00CC66">align=1</span>,
           |  nvsize=21, nvalign=1]
</pre>

This is an example of how Clang helps to visualize the memory layout.

As a bonus, this feature may be used for educational purposes, i.e. to understand how virtual methods and inheritance affect memory layout.
