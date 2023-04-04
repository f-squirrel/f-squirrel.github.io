---
title: The "moving" truth behind std::optional
published: true
permalink: "/the-state-of-std-optional-after-move/"
tags: [cpp, optional, move]
share-title: The "moving" truth behind std::optional
share-img: /img/optional-mandatory-crop.png
share-description: Unexpected behavior of std::optional after move
readtime: true
---


The class template `std::optional` manages an optional contained value, i.e. a value that may or may not be
present[[1]](https://en.cppreference.com/w/cpp/utility/optional){:target="_blank"}. It is a great alternative for `std::unique_ptr`, or
raw pointer, when used solely to express that a value is indeed optional. The type of the variable explicitly states that a contained variable is optional and it is stored by value.

The following is an example of a function that returns a value, if it exists, or null optional:

```cpp
std::optional<Object> get();

auto object = get();
if (object) {
    // Use object
}
```

Since C++ 11 allows the "movement" of variables, the below demonstrates what happens when `std::optional` is moved:

```cpp
template <typename T>
void print(const T &o, const std::string &name) {
    std::cout << name << ".has_value(): "
              << std::boolalpha
              << o.has_value()
              << std::noboolalpha << std::endl;
    if (o.has_value()) {
        std::cout << name
              << ".value(): '" << o.value() << "'"
              << std::endl
              << std::endl;
    }
}

int main(int argc, char *argv[]) {
    auto i1 = std::make_optional<int>(42);
    auto i2 = std::move(i1);
    print(i1, "i1");
    print(i2, "i2");
    auto s1 = std::make_optional<std::string>("hello");
    auto s2 = std::move(s1);
    print(s1, "s1");
    print(s2, "s2");
    return 0;
}
```

Output:

<pre>
i1.has_value(): true
i1.value(): '42'

i2.has_value(): true
i2.value(): '42'

<span style="background-color: #FFFF00">
s1.has_value(): true
s1.value(): ''
</span>

s2.has_value(): true
s2.value(): 'hello'
</pre>

This example portrays how `std::move` does not change the state of `optional` if the contained value is primitive, as
`move` for primitives just copies the values. For objects like `std::string`, it actually moves the value (and empties the
string contained in `s1`) but `s1` still `has_value`.  
The following visuals illustrate the various states of `std::optional<std::string>` discussed thus far.

![std::optional layout](/img/optional.svg)

[cppreference.com](https://en.cppreference.com/w/cpp/utility/optional/optional){:target="_blank"} provides a clear explanation of the behavior:
> Move constructor: If other contains a value, initializes the contained value as if direct-initializing (but not
> direct-list-initializing) an object of type T with the expression std::move(*other) and does not make other
> empty: a moved-from optional still contains a value, but the value itself is moved from. If other does not contain a
> value, constructs an object that does not contain a value.
> This constructor does not participate in overload resolution unless `std::is_move_constructible_v<T>` is true.
> It is a trivial constructor if `std::is_trivially_move_constructible_v<T>` is true.

The behavior is perfectly legal in terms of the standard, but can seem illogical from the user's perspective, who would naturally expect the optional to have no value if it has been moved.
In order to eliminate all doubt, I would suggest to `std::optional::reset` after moving an optional object.  
The reset would call the destructor of the contained object (if the destructor exists) and set the `has_value` flag to `false`.

```cpp
    i1.reset();
    print(i1, "i1");
    s1.reset();
    print(s1, "s1");
```

Output:

```plain
i1.has_value(): false
s1.has_value(): false
```

## Update

Another option is to avoid using variables after they are moved out because, in many cases, it is bug-prone.
C++ compilers do not warn users if a variable is used after a move. The only alternative I am familiar with is Clang-Tidy with [bugprone-use-after-move](https://clang.llvm.org/extra/clang-tidy/checks/bugprone/use-after-move.html) check turned on.

*Special thanks to [Petar Ivanov](https://www.linkedin.com/in/petar-ivanov-37840224/) for the idea and [Rina Volovich](https://www.linkedin.com/in/rina-volovich/) for editing.*

Please share your thoughts on [Twitter](https://twitter.com/dbdanilov/status/1321880543315845122?s=20) or [LinkedIn](https://www.linkedin.com/posts/ddanilov_cpp-cplusplus-activity-6727646045565661184-yVaZ?utm_source=share&utm_medium=member_desktop).
