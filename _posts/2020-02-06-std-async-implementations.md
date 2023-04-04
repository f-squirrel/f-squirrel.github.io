---
title: Implementations of std::async and how they might Affect Applications
published: true
permalink: "/std-async-implementations/"
tags: [cpp, async]
share-description: "Description of major implementations of std::async, when it uses thread pool and when does not"
readtime: true
---


## Intro ##

Recently, I have been reviewing the function `std::async` and decided to get a better understanding.

The C++ standard says:
> The function template async provides a mechanism to launch a function potentially in a new thread and provides the
> result of the function in a future object with which it shares a shared state.

Here is a simple example of usage of `std::async`:

```cpp
#include <iostream>
#include <future>

int foo(int a) {
    //some task to run in async
    return a;
}

int main() {
    std::future<int> f = std::async(&foo, 10);
    std::cout << f.get() << std::endl;
    return 0;
}
```

There are two launch policies, which define whether a function is launched in a separate thread or not:
`std::launch::async` and `std::launch::deferred`.

* If the `async` flag is set, then a callable function will be executed in a separate thread.
* If the `deferred` flag is set, a callable function will be stored together with its arguments, but the
`std::async` function __will not launch a new thread__. Moreover, the callable function will be executed if either
`future::get()` or `future::wait()` is called.
* If neither of flags is set, it is up to the implementation to decide which policy to choose.

## In Essence, what is the Default Policy? ##

Let us understand which policy is the default in the major implementations of the C++ standard library.

In __GCC__, the [default option](https://github.com/gcc-mirror/gcc/blob/a1c9c9ff06ab15e697d5bac6ea6e5da2df840cf5/libstdc%2B%2B-v3/include/std/future){:target="_blank"} is `launch::async|launch::deferred`:

```cpp
/// async, potential overload
template<typename _Fn, typename... _Args>
  _GLIBCXX_NODISCARD inline future<__async_result_of<_Fn, _Args...>>
  async(_Fn&& __fn, _Args&&... __args)
  {
    return std::async(launch::async|launch::deferred,
    std::forward<_Fn>(__fn),
    std::forward<_Args>(__args)...);
  }
```

In actuality, the chosen policy will be  `launch::async` (lines 11 and 27):

```cpp
  /// async
  template<typename _Fn, typename... _Args>
    _GLIBCXX_NODISCARD future<__async_result_of<_Fn, _Args...>>
    async(launch __policy, _Fn&& __fn, _Args&&... __args)
    {
      std::shared_ptr<__future_base::_State_base> __state;
      if ((__policy & launch::async) == launch::async)
 {
   __try
     {
       __state = __future_base::_S_make_async_state(
    std::thread::__make_invoker(std::forward<_Fn>(__fn),
           std::forward<_Args>(__args)...)
    );
     }
#if __cpp_exceptions
   catch(const system_error& __e)
     {
       if (__e.code() != errc::resource_unavailable_try_again
    || (__policy & launch::deferred) != launch::deferred)
  throw;
     }
#endif
 }
      if (!__state)
 {
   __state = __future_base::_S_make_deferred_state(
       std::thread::__make_invoker(std::forward<_Fn>(__fn),
       std::forward<_Args>(__args)...));
 }
      return future<__async_result_of<_Fn, _Args...>>(__state);
    }
```

[__LLVM__](https://github.com/llvm-mirror/libcxx/blob/78d6a7767ed57b50122a161b91f59f19c9bd0d19/include/future){:target="_blank"}
 has a special launch policy, `launch::any`, for the default option:

```cpp
template <class _Fp, class... _Args>
_LIBCPP_NODISCARD_AFTER_CXX17 inline _LIBCPP_INLINE_VISIBILITY
future<typename __invoke_of<typename decay<_Fp>::type, typename decay<_Args>::type...>::type>
async(_Fp&& __f, _Args&&... __args)
{
    return _VSTD::async(launch::any, _VSTD::forward<_Fp>(__f),
                                    _VSTD::forward<_Args>(__args)...);
}
```

It is defined as a combination of `launch::async` and `launch::deferred`.

```cpp
enum class launch
{
    async = 1,
    deferred = 2,
    any = async | deferred
};
```

However, the actual selection will be `launch::async` (lines 13 and 21):

```cpp
template <class _Fp, class... _Args>
_LIBCPP_NODISCARD_AFTER_CXX17
future<typename __invoke_of<typename decay<_Fp>::type, typename decay<_Args>::type...>::type>
async(launch __policy, _Fp&& __f, _Args&&... __args)
{
    typedef __async_func<typename decay<_Fp>::type, typename decay<_Args>::type...> _BF;
    typedef typename _BF::_Rp _Rp;

#ifndef _LIBCPP_NO_EXCEPTIONS
    try
    {
#endif
        if (__does_policy_contain(__policy, launch::async))
        return _VSTD::__make_async_assoc_state<_Rp>(_BF(__decay_copy(_VSTD::forward<_Fp>(__f)),
                                                     __decay_copy(_VSTD::forward<_Args>(__args))...));
#ifndef _LIBCPP_NO_EXCEPTIONS
    }
    catch ( ... ) { if (__policy == launch::async) throw ; }
#endif

    if (__does_policy_contain(__policy, launch::deferred))
        return _VSTD::__make_deferred_assoc_state<_Rp>(_BF(__decay_copy(_VSTD::forward<_Fp>(__f)),
                                                        __decay_copy(_VSTD::forward<_Args>(__args))...));
    return future<_Rp>{};
}
```

The same can be concluded for [__MSVC's__](https://github.com/microsoft/STL/blob/b3504262fe51b28ca270aa2e05146984ef758428/stl/inc/future){:target="_blank"} default policy:

```cpp
template <class _Ret, class _Fty>
_Associated_state<typename _P_arg_type<_Ret>::type>* _Get_associated_state(
    launch _Psync, _Fty&& _Fnarg) { // construct associated asynchronous state object for the launch type
    switch (_Psync) { // select launch type
    case launch::deferred:
        return new _Deferred_async_state<_Ret>(_STD forward<_Fty>(_Fnarg));
    case launch::async: // TRANSITION, fixed in vMajorNext, should create a new thread here
    default:
        return new _Task_async_state<_Ret>(_STD forward<_Fty>(_Fnarg));
    }
}
```

It can therefore be confirmed that, at least for now, all three implementations **have the same default launch policy,
which is `launch::async`.

## Using std::async with Default Options ##

If the default option is used, it is unknown which policy will be chosen. “So what? The compiler knows better!” you may
say.

But what will happen if a callable function locks mutexes or stores variables with the
[_thread_local_](https://en.cppreference.com/w/cpp/keyword/thread_local){:target="_blank"} storage duration?
A change in the default mode might then affect your application dramatically.
If the default policy is changed from `async` to `deferred`,

* The `thread_local` variables will use the values from the previous executions of a callable function.
* The locking of mutexes inside of a callable function may lead to a deadlock.

In the opposite scenario where there is a switch from `deferred` to `async`, then objects used by a callable function
may get shared with other threads, which may lead to data races.

## How does `std::launch::async` Work in Different Implementations? ##

For now, we know that if no policy is specified, then `std::async` launches a callable function in a separate thread.
However, the C++ standard does not specify whether the thread is a new one or reused from a thread pool.
Let us see how each of the three implementations launches a callable function.

__GCC__ calls `__future_base::_S_make_async_state`, which creates an instance of `_Async_state_impl`. Its constructor launches a new `std::thread` (line 12):

```cpp
// Shared state created by std::async().
// Starts a new thread that runs a function and makes the shared state ready.
template<typename _BoundFn, typename _Res>
  class __future_base::_Async_state_impl final
  : public __future_base::_Async_state_commonV2
  {
  public:
    explicit
    _Async_state_impl(_BoundFn&& __fn)
    : _M_result(new _Result<_Res>()), _M_fn(std::move(__fn))
    {
  _M_thread = std::thread{ [this] {
      __try
        {
   _M_set_result(_S_task_setter(_M_result, _M_fn));
        }
      __catch (const __cxxabiv1::__forced_unwind&)
        {
   // make the shared state ready on thread cancellation
   if (static_cast<bool>(_M_result))
     this->_M_break_promise(std::move(_M_result));
   __throw_exception_again;
        }
      } };
    }
```

__LLVM__ calls `_VSTD::__make_async_assoc_state`, which does the same: launches a new `std::thread`(line 11):

```cpp
template <class _Rp, class _Fp>
future<_Rp>
#ifndef _LIBCPP_HAS_NO_RVALUE_REFERENCES
__make_async_assoc_state(_Fp&& __f)
#else
__make_async_assoc_state(_Fp __f)
#endif
{
    unique_ptr<__async_assoc_state<_Rp, _Fp>, __release_shared_count>
        __h(new __async_assoc_state<_Rp, _Fp>(_VSTD::forward<_Fp>(__f)));
    _VSTD::thread(&__async_assoc_state<_Rp, _Fp>::__execute, __h.get()).detach();
    return future<_Rp>(__h.get());
}
```

__MSVC__ creates an instance of `_Task_async_state`, which creates a concurrency task and passes a callable function there:

```cpp
// CLASS TEMPLATE _Task_async_state
template <class _Rx>
class _Task_async_state : public _Packaged_state<_Rx()> {
    // class for managing associated synchronous state for asynchronous execution from async
public:
    using _Mybase     = _Packaged_state<_Rx()>;
    using _State_type = typename _Mybase::_State_type;

    template <class _Fty2>
    _Task_async_state(_Fty2&& _Fnarg) : _Mybase(_STD forward<_Fty2>(_Fnarg)) {
        _Task = ::Concurrency::create_task([this]() { // do it now
            this->_Call_immediate();
        });

        this->_Running = true;
    }
```

`::Concurrency::create_task` is part of [Microsoft’s Parallel Patterns
Library](https://docs.microsoft.com/en-us/cpp/parallel/concrt/parallel-patterns-library-ppl?view=vs-2019){:target="_blank"}.
According to [MSDN](https://docs.microsoft.com/en-us/cpp/parallel/concrt/task-parallelism-concurrency-runtime?view=vs-2019){:target="_blank"},
_the `task` class uses the __Windows ThreadPool__ as its scheduler, not the Concurrency Runtime_.

I assume that Microsoft engineers decided to use the thread pool because thread creation is a relatively heavy
operation. Additionally, spawning many threads may exhaust the operating system.

## Summary ##

The bottom line is that `std::async` is a very useful feature. It makes asynchronous operations simpler and the code
much cleaner, though a few potential problems can be identified:

* Despite the fact that all major implementations use `launch::async` by default, this is not guaranteed, according
to the standard. Theoretically, this may change in the future.
* Launching `std::async` in `launch::async` mode is relatively expensive because of thread creation (GCC and
LLVM) and eventually, may lead to resource exhaustion due to many `std::async` spawned in parallel.
* In the MSVC case, `thread_local` variables may not be properly initialized, as MSVC reuses threads from _Windows
ThreadPool_ instead of launching new ones.
* MSVC creates a thread pool the first time you run `std::async`, which may become an overhead in certain
situations.
* The difference in the implementations may lead to unexpected behavior after the code is ported between GCC/LLVM and
MSVC.

Please share your thoughts on [LinkedIn](https://www.linkedin.com/posts/ddanilov_implementations-of-stdasync-and-how-they-activity-6633020851841183744-u-d_?utm_source=share&utm_medium=member_desktop).