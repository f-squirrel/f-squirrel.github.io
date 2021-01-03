---
title: std::shared_ptr is an anti-pattern
subtitle: In most of the cases
published: true
permalink: "/shared-ptr-is-evil/"
tags: [C++, shared_ptr]
readtime: true
---

## A bit of history ##
The first time a shared pointer was introduced in boost library in the year of 1999.
Even before [boost had release numbers](https://www.boost.org/users/history/old_versions.html)!
Back then the only alternative standard C++ could provide with was `std::auto_ptr`.
 It was so bad that it was rarely used, [got deprecated in C++ 11](http://www.open-std.org/jtc1/sc22/wg21/docs/papers/2011/n3242.pdf) and [was eventually removed in C++ 17](https://en.cppreference.com/w/cpp/memory/auto_ptr).
So the only way to have a nice smart pointer in C++ 98/03 was to use `boost::shared_ptr`.
In C++ 11 `boost::shared_ptr` finally made it to the standard library.


## A few words about how `shared_ptr` works ##
If the reader knows how `std::shared_ptr` works they may skip this section.
From a very simplified point of view, a shared pointer has two pointers: one to an object in the heap that it owns and
another to a reference counter of shared instances.
* Every time a shared pointer’s copy-constructor is called the counter is incremented
* Every time a shared pointer’s assignment operator is called the counter of the right-hand pointer is incremented and
of the left-hand is decremented
* Every time a shared pointer’s destructor is called the counter is decremented
* If the counter equals zero the object is deleted


## The ideal usage ##
With the shared pointer a programmer may create a variable and pass it wherever they want without caring about memory
deallocation because at some point it happens automatically. It is awesome, isn’t it? Let us see!


## Incrementation of the reference counter and multi-threading ##
In order to satisfy thread-safety requirements, the reference counter is usually implemented as atomic. So every time a
shared pointer is passed by value its atomic counter is incremented and decremented.
Obviously, an incrementation of the atomic counter is [relatively
expensive](https://stackoverflow.com/questions/32591204/how-expensive-are-atomic-operations).

This issue may be addressed by *passing shared pointers by const reference*. Thus, you do not increment/decrement the counter and save CPU cycles. Let’s see the difference in performance by running the following code:
```cpp
#include <memory>

using namespace std::chrono;
using shared_ptr_t=std::shared_ptr<int>;

void shared_ptr_receiver_by_value(shared_ptr_t ptr) {
    (void)*ptr;
}

void shared_ptr_receiver_by_ref(const shared_ptr_t& ptr) {
    (void)*ptr;
}

void test_copy_by_value(uint64_t n) {
    auto ptr = std::make_shared<int>(100);
    for(uint64_t i = 0u; i < n; ++i) {
        shared_ptr_receiver_by_value(ptr);
    }
}

void test_copy_by_ref(uint64_t n) {
    auto ptr = std::make_shared<int>(100);
    for(uint64_t i = 0u; i < n; ++i) {
        shared_ptr_receiver_by_ref(ptr);
    }
}

int main(int argc, char *argv[]) {
    uint64_t n = (argc == 3 ) ? std::stoull(argv[2]) : 100;
    auto t1 = high_resolution_clock::now();
    if(atoi(argv[1]) == 1) {
        test_copy_by_value(n);
    } else {
        test_copy_by_ref(n);
    }
    auto t2 = high_resolution_clock::now();
    auto time_span = duration_cast<duration<int64_t, std::micro>>(t2 - t1);
    std::cout << "It took me " << time_span.count() << " microseconds.\n";
    return 0;
}
```

And the output is:
<pre>
$ ./cpu_atomic_copy.bin 1 999999
It took me 3616 microseconds.
$ ./cpu_atomic_copy.bin 2 999999
It took me 2 microseconds.
</pre>

The difference is breathtaking!!!<br> But wait, if I do not update the reference counter my shared pointer does not
track the reference count!  Shared pointer’s counter can be changed outside of the current function scope only if there
is a reference to the pointer in another thread.  Thus, **the only reason to pass a shared pointer by value is when
passing it to another thread**.  So when the first thread stops and all its objects are destructed or the thread just
stops using the pointer then the shared pointer’s atomic counter is decremented. But the stored object is not deleted
and the other thread(s) may continue using it.  However, we usually join threads in the end. So that we know when they
stop and do not need any of the resources shared with them.  It means that you probably need to pass a shared pointer to
a thread only if you run a detached thread.  Or the thread that passes shared_ptr does not use it anymore. Though, in
this case, you might prefer using `std::unique_ptr`.


## Shared pointer initialization ##
This is the initialization of std::shared_ptr I usually see:

```cpp
// let us assume that new int(42) does not throw
auto ptr = std::shared_ptr<int>(new int(42));
```

What happens in the code?
1. Memory in the heap is allocated for the integer 42 and its pointer is stored in the shared pointer’s *stored pointer*
1. Memory in the heap is allocated for the reference counter and its pointer is stored in the second inner pointer of
   the shared pointer

Why is it bad?
1. You make two memory allocations in the heap for one stored object
1. The shared pointer’s data is located in two different parts of the heap. Which potentially may lead to a higher cache
   miss rate. However, I haven’t succeeded to prove it yet


How to fix it?
`std::make_shared` comes to help:


```cpp
auto ptr = std::make_shared<int>(42);
```


It looks almost the same but this code makes __only one allocation of a contiguous piece of memory used for storing both
the stored object and the reference counter__.
The picture below shows the difference in the memory layout of shared pointers created in the two ways.


<p align="center">
  <img src="/img/shared_ptr_memory_map.png" title="Shared pointer memry layout">
</p>


Let us check the memory allocation with the following simple code:
```cpp
#include <iostream>
#include <memory>
#include <vector>


void test_shared_ptr(size_t n) {
    std::cout << __FUNCTION__ << "\n";
    std::vector<std::shared_ptr<size_t>> v;
    v.reserve(n);
    for(size_t i = 0u; i < n; ++i) {
        v.push_back(std::shared_ptr<size_t>(new size_t(i)));
    }
}

void test_make_shared(size_t n) {
    std::cout << __FUNCTION__ << "\n";
    std::vector<std::shared_ptr<size_t>> v;
    v.reserve(n);
    for(size_t i = 0u; i < n; ++i) {
        v.push_back(std::make_shared<size_t>(i));
    }
}

int main(int argc, char *argv[]) {
    size_t n = (argc == 3 ) ? atoi(argv[2]) : 100;
    if(atoi(argv[1]) == 1) {
        test_shared_ptr(n);
    } else {
        test_make_shared(n);
    }
    return 0;
}
```

Let’s check it with valgrind:
<pre>
$valgrind --tool=memcheck ./memory_allocation.bin 1 100000
==3005== Memcheck, a memory error detector
==3005== Copyright (C) 2002-2017, and GNU GPL'd, by Julian Seward et al.
==3005== Using Valgrind-3.13.0 and LibVEX; rerun with -h for copyright info
==3005== Command: ./memory_allocation.bin 1 100000
==3005==
test_shared_ptr
==3005==
==3005== HEAP SUMMARY:
==3005==     in use at exit: 0 bytes in 0 blocks
==3005==   total heap usage: <span style="background-color: #FFFF00">200,003 allocs</span>, 200,003 frees, 4,873,728 bytes allocated
==3005==
==3005== All heap blocks were freed -- no leaks are possible
==3005==
==3005== For counts of detected and suppressed errors, rerun with: -v
==3005== ERROR SUMMARY: 0 errors from 0 contexts (suppressed: 0 from 0)


$valgrind --tool=memcheck ./memory_allocation.bin 2 100000
==3010== Memcheck, a memory error detector
==3010== Copyright (C) 2002-2017, and GNU GPL'd, by Julian Seward et al.
==3010== Using Valgrind-3.13.0 and LibVEX; rerun with -h for copyright info
==3010== Command: ./memory_allocation.bin 2 100000
==3010==
test_make_shared
==3010==
==3010== HEAP SUMMARY:
==3010==     in use at exit: 0 bytes in 0 blocks
==3010==   total heap usage: <span style="background-color: #FFFF00">100,003 allocs</span>, 100,003 frees, 4,073,728 bytes allocated
==3010==
==3010== All heap blocks were freed -- no leaks are possible
==3010==
==3010== For counts of detected and suppressed errors, rerun with: -v
==3010== ERROR SUMMARY: 0 errors from 0 contexts (suppressed: 0 from 0)
</pre>


As you may see there are **200,003 allocs** while using constructor versus **100,003 allocs** with `std::make_shared`.
[More info about `std::make_shared`](https://en.cppreference.com/w/cpp/memory/shared_ptr/make_shared)


## Reference cycles ##
Consider the next piece of code:
```cpp
#include <iostream>
#include <memory>


struct B;

struct A {
    ~A() { std::cout << "dtor ~A\n"; }
    std::shared_ptr<B> b;
};

struct B {
    ~B() { std::cout << "dtor ~B\n"; }
    std::shared_ptr<A> a;
};

void test() {
    auto ptrA = std::make_shared<A>();
    auto ptrB = std::make_shared<B>();
    ptrA->b = ptrB;
    ptrB->a = ptrA;
}
int main() {
    test();
    return 0;
}
```

Output:
<pre>
$ ./reference_cycle.bin
$
</pre>

Its output is empty which means the destructors are never called due to a reference cycle:
`ptrA` points to `ptrB` and vice versa.
Of course, the code is naive and a programmer may easily find and break the cycle.
But if you have multiple shared objects passing from one
class to another you can get in this situation.
However, the problem of reference cycles is not new and in order to address
it the C++ standard library provides `std::weak_ptr`.

```cpp
struct B_;

struct A_ {
    ~A_() { std::cout << "dtor ~A_\n"; }
    std::shared_ptr<B_> b;
};

struct B_ {
    ~B_() { std::cout << "dtor ~B_\n"; }
    // one of the pointers becomes weak
    std::weak_ptr<A_> a;
};

void test_() {
    auto ptrA = std::make_shared<A_>();
    std::cout << "A address: " << ptrA.get() << std::endl;
    auto ptrB = std::make_shared<B_>();
    ptrA->b = ptrB;
    ptrB->a = ptrA;
    std::cout << "Number of references to A: "
              << ptrB->a.use_count()
              << std::endl
              << "A address: "
              << ptrB->a.lock().get()
              << std::endl;
}

int main() {
    test_();
    return 0;
}
```

The output shows that all the destructors are called now:

<pre>
$ ./reference_cycle.bin
A address: 0x7f83564017d8
Number of references to A: 1
A address: 0x7f83564017d8
dtor ~A_
dtor ~B_
</pre>

[More info about `std::weak_ptr`](https://en.cppreference.com/w/cpp/memory/weak_ptr)

## `std::make_shared` together with `std::weak_ptr` ##
So now we know that `std::make_shared` saves memory allocation and `std::weak_ptr` may prevent reference cycles.
But what happens when we use them together?

```cpp
struct A_ {
    // Let us add char buffer str to class A_
    char str[256];
    A_() {
      strcpy(str, "AAAAAAAAA");
    }
    ~A_() { std::cout << "dtor ~A_\n"; }
    std::shared_ptr<B_> b;
};

struct B_ {
    ~B_() { std::cout << "dtor ~B_\n"; }
    std::weak_ptr<A_> a;
};

std::tuple<std::weak_ptr<A_>,A_*>
create_make_shared_and_return_weak_ptr() {
    auto ptrA = std::make_shared<A_>();
    std::cout << "A_ address: " << ptrA.get() << std::endl
              << "A_::str = " << ptrA->str
              << std::endl;
    auto ptrB = std::make_shared<B_>();
    ptrA->b = ptrB;
    ptrB->a = ptrA;
    std::cout << "Number of references to A_: " << ptrB->a.use_count()
        << std::endl;
    return {ptrB->a, ptrB->a.lock().get()};
}

void test_make_shared_with_weak() {
    auto[weak, raw_ptr] = create_make_shared_and_return_weak_ptr();
    std::cout << "Returned from create_make_shared_and_return_weak_ptr"
              << std::endl;
    auto strong = weak.lock();
    std::cout << "Number of references to A_: "
              << strong.use_count()
              << std::endl;
    std::cout << "Stored address: " << strong.get() << std::endl;
    std::cout << "Value of A_::str stored by original address of A_: "
              << raw_ptr
              << " is: " << raw_ptr->str << std::endl;
}

int main() {
    test_make_shared_with_weak();
    return 0;
}
```

In the code above the function `create_make_shared_and_return_weak_ptr` creates two shared pointers and then returns
a copy of the weak pointer `ptrB->a` together with the raw pointer to the same object(it does not affect the results
but we need it further).
What do you expect to happen after the flow returns from `create_shared_and_return_weak_ptr`?
We know that there were no reference cycles in the function and after exiting there are no instances of shared pointers.
I’d expect the stored objects to be destructed and the memory released. But let’s see what happens:


<pre>
$ ./reference_cycle.bin
A_ address: 0x7fd37ac01918
A_::str = AAAAAAAAA
Number of references to A_: 1
dtor ~A_
dtor ~B_
Returned from create_make_shared_and_return_weak_ptr
Number of references to A_: 0
Stored address: 0x0
Value of A_::str stored by original address of A_: 0x7fd37ac01918 is: AAAAAAAAA
</pre>

As you see the destructors are called and the weak pointer received from function
`create_make_shared_and_return_weak_ptr` holds a null pointer which is expected.  Nevertheless, the value stored in
the variable `A_::str` is still available by its original address(`0x7fc71ec01878`). How come???


Here is the explanation from [cppreference.com](https://en.cppreference.com/w/cpp/memory/shared_ptr/make_shared):
> If any std::weak_ptr references the control block created by std::make_shared after the lifetime of all shared owners
> ended, the memory occupied by T persists until all weak owners get destroyed as well, which may be undesirable if
> sizeof(T) is large.

Basically it means that the destructor `~A_()` is called explicitly but `delete` for the stored pointer is not!
The reason is that a shared pointer created with `std::make_shared` stores both the stored object and control block in a
contiguous piece of memory.
As a result, the two can be deleted only together.
But if there is any weak pointer the control block cannot be deleted otherwise the weak pointer would not have
information about the reference count.
So *the solution is to destruct the stored object but not to delete the whole memory block until there are weak pointers*.

Thus, C++ provides two very nice features but using them together may lead to inefficient code.
However, if you use `std::weak_ptr` it might be a good idea to reconsider the design of your application.

## Design problems ##

So, for now, we know that shared pointer is relatively expensive to copy by value, requires specific instantiation to
save memory allocations, and in the case of incorrect usage may lead to cycle references.
And we already know most of the problems can be addressed by using the right language constructions.

Despite this, there is another problem - design. In general, C++ is a language that expects a programmer to have full
control of used resources and objects’ lifecycles. Shared pointers make the application’s memory model more complex and
couplings between its parts are hard to track. Thus, the whole application becomes more bug-prone.


## Conclusion ##
After reviewing all the cases above I came to the conclusion:
* Try to follow the single ownership principle
* Prefer to use objects with automatic storage duration
* If you need a pointer, try using `unique_ptr`
* If you have to use `shared_ptr` make sure you don’t overuse it and keep in mind it’s features


## Credits ##
Thanks to [Sergey Pastukhov](https://www.linkedin.com/in/spastukhov/) and [Orian
Zinger](https://www.linkedin.com/in/orian-zinger/) for help.
