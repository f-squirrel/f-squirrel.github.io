---
title: How I solved a bug by disabling C++ extensions
published: true
permalink: "/default-non-standard-features/"
share-img: /img/fine-print.png
share-description: "Tips on how to make your compiler strictly follow the C++ standard"
tags: [cpp, variable-length array, gcc, clang]
readtime: true
comments: false
---

I was recently investigating a bug and would like to share an unexpected, yet interesting discovery regarding the cause of the issue.
In order to provide an idea of what I was working on, consider the following code as a simplified representation:

```cpp
void read_to_buffer(std::size_t size) {
    char buffer[size];
    std::memset(buffer, 0, size);
    // call some function to fill the buffer
    // process the content of the buffer

    // to make sure that the function is not compiled out
    std::cout << buffer << std::endl;
}

int main(int argc, char *argv[]) {
    auto size = std::stoul(argv[1]);
    read_to_buffer(size);
    return 0;
}
```

In my case, the code was working perfectly, until the `size` was changed somewhere deep in the configuration. After that, it started to crash due to a segmentation fault, as the code was trying to allocate too much memory on stack. The solution is to simply allocate the memory on heap. Fortunately, C++ provides plenty of options, such as `std::vector v(size)`, `std::string s(size, 0)`, `std::unique_ptr(new char[size])`, and so forth.

My only question was, "How was the code successfully compiled in the first place?"

According to the C++ standard, the size of objects allocated on stack must be known at compile time. In the sample I mentioned above, `char buffer[size]` is a [variable-length array](https://en.cppreference.com/w/c/language/array){:target="_blank"}, which is actually a feature from the C99 standard, and not related to C++.
The interesting catch is that while variable-length array is not supported by the C++ standard, GCC and clang still attempt to compile it because they innately comply with both standards.
I can offer the following recommendation in order to avoid such a subtle nuisance: the variable-length arrays need to be explicitly disabled by adding `-Werror=vla` to the `CXX_FLAGS`.

Although the optimal and safest solution would be to use the `-pedantic` flag, which instructs the compiler to adhere to the C++ standard and forbidding all extensions.

And now, ladies and gentlemen,

Exhibit A:

<pre>
$make
/home/user/example/main.cpp:7:16: <span style="background-color: #FFFF00">error: variable length arrays are a C99 feature [-Werror,-Wvla-extension]</span>
    char buffer[size];
               ^
1 error generated.
</pre>

Exhibit B:

<pre>
$make
/home/user/example/main.cpp:7:21: <span style="background-color: #FFFF00">error: ISO C++ forbids variable length array ‘buffer’ [-Werror=vla]</span>
     char buffer[size];
                     ^
cc1plus: all warnings being treated as errors
</pre>

Please share your thoughts on [LinkedIn](https://www.linkedin.com/posts/ddanilov_cpplus-activity-6707398283444146176-raKg?utm_source=share&utm_medium=member_desktop).
