---
title: Using shared_ptr for reloadable config
published: true
permalink: "/usage-of-shared_ptr"
tags: [cpp, shared_ptr]
readtime: true
share-img: /img/cpp_logo.png
share-description: How to use std::shared_ptr for implementing reloadable configs
comments: false
---

After my post [std::shared_ptr is an anti-pattern](/shared-ptr-is-evil/) I was thinking about an example of justified usage of `std::shared_ptr`. I have mentioned that it can be used to implement caching, and in this post, I am going to use it for implementing reloadable config.

Imagine a situation when you need to develop an object shared across the system that can be arbitrarily updated. For example, there is a class representing a configuration of the application: it is created on the startup of the program and later updated via a watcher thread.

The fact that the configuration object is used by many users, potentially located in multiple threads and updated in the watcher thread requires synchronization of the config.

The straightforward approach is to create a `Config` class with an inner mutex, locking on each access via getters and setters. However, this approach leads to quite a lot of work with mutexes which might be tedious and, often, bug-prone. Instead, I am going to use the atomic features of the shared pointer.

First of all, I am going to create a class `InnerConfig` representing the actual data (`name_` and `version_`) to be shared with the rest of the code.

```cpp
class NonCopyable {
public:
    NonCopyable()                          = default;
    NonCopyable(NonCopyable &&)            = default;
    NonCopyable &operator=(NonCopyable &&) = default;

    NonCopyable(const NonCopyable &)            = delete;
    NonCopyable &operator=(const NonCopyable &) = delete;

    ~NonCopyable() = default;
};

class InnerConfig : private NonCopyable {
public:
    InnerConfig(const std::string &name, u_int16_t version)
        : name_{name}, version_{version} {}

    [[nodiscard]] const std::string &name() const { return name_; }
    [[nodiscard]] uint16_t version() const { return version_; }

private:
    std::string name_;
    uint16_t version_;
};
```

As you can see, the class can be initialized once and its state can be received only via constant getters. Additionally, this class is non-copyable (the `NonCopyable` can be replaced with `boost::noncopyable`).

This class is owned by another class `ConfigManager` responsible for loading the `InnerConfig` from the filesystem and updating its value if the config file has changed. This class is not only non-copyable but also not movable. It is my personal preference, some developers might prefer to leave the option to move it.

```cpp
class ConfigManager : private NonCopyable {
public:
    ConfigManager(const std::string config_path,
                  std::chrono::milliseconds reload_interval)
        : config_path_{config_path}, reload_interval_{reload_interval} {

        last_used_ = std::move(read().value());
        auto new_ptr =
            std::make_shared<const InnerConfig>(std::move(parse(last_used_).value()));
        std::atomic_store(&inner_config_, new_ptr);

        stop_    = false;
        watcher_ = std::thread([&]() { watch(); });
    }

    ~ConfigManager() {
        stop_ = true;
        watcher_.join();
    }

    ConfigManager(ConfigManager &&)            = delete;
    ConfigManager &operator=(ConfigManager &&) = delete;

    Config get() const { return Config{inner_config_}; }

private:
    void watch() {
        for (; !stop_; std::this_thread::sleep_for(reload_interval_)) {
            auto latest = read();
            if (latest != last_used_) {
                auto inner_config = parse(latest.value());
                if (!inner_config.has_value()) {
                    continue;
                }
                auto new_ptr = std::make_shared<const InnerConfig>(
                    std::move(inner_config.value()));
                std::atomic_store(&inner_config_, new_ptr);
                last_used_ = std::move(latest.value());
            }
        }
    }

    [[nodiscard]] std::optional<std::string> read() const {
        auto file = std::ifstream{config_path_.c_str()};
        if (!file.is_open()) {
            return {};
        }
        auto latest = std::string{
            (std::istreambuf_iterator<std::string::value_type>(file)),
            std::istreambuf_iterator<std::string::value_type>()};
        file.close();
        return {latest};
    }

    [[nodiscard]] std::optional<InnerConfig> parse(const std::string &) const {
        // TODO: implement deserialization from string
        return {InnerConfig{"name", 42}};
    }

private:
    const std::string config_path_;
    const std::chrono::milliseconds reload_interval_;

    std::shared_ptr<const InnerConfig> inner_config_;
    std::string last_used_;
    std::atomic_bool stop_ = true;
    std::thread watcher_;
```

The class `ConfigManager` loads configuration data from a file and periodically reloads it. The loaded configuration data is stored in a `std::shared_ptr` of type `InnerConfig` that can be accessed through a `Config` object.

The `ConfigManager` constructor takes two parameters: the path to the configuration file and a time interval for reloading the configuration data. Upon construction, the `ConfigManager` reads the initial configuration data from the file and stores it in the `last_used_` string. It then parses the initial configuration data and creates a `std::shared_ptr` of type `InnerConfig`, which is stored in the `inner_config_` member variable. Finally, it starts a background thread (`watcher_`) that periodically checks if the configuration file has been updated and, if so, reloads the configuration data and updates the `inner_config_` member variable.

The `ConfigManager` class provides a `get()` method that returns a `Config` object that provides read-only access to the current configuration data stored in `inner_config_` through methods `name()` and `version()`.

```cpp
class Config {
public:
    Config(const std::shared_ptr<const InnerConfig> &inner_config)
        : inner_config_{inner_config} {}

    [[nodiscard]] std::string name() const { return load()->name(); }
    [[nodiscard]] uint16_t version() const { return load()->version(); }

private:
    [[nodiscard]] const std::shared_ptr<const InnerConfig> load() const {
        auto ptr = std::atomic_load(&inner_config_);
        assert(ptr);
        return ptr;
    }

    const std::shared_ptr<const InnerConfig> &inner_config_;
};
```

The important feature of this code is the use of `std::atomic_store` and `std::atomic_load` functions to safely update the `InnerConfig` object with the latest configuration values. These functions provide atomic operations that ensure that the shared data is accessed in a thread-safe manner. Specifically, `std::atomic_store` atomically stores a new value to a shared variable, and `std::atomic_load` atomically loads the current value of a shared variable.

In the `watch` function, when a new configuration is read from the file, it is parsed into an `InnerConfig` object and stored in a new shared pointer. This shared pointer is then atomically loaded and stored using `std::atomic_store`. The `Config` object accesses the loaded `InnerConfig` object through the `load` function, which uses `std::atomic_load` to safely access the shared pointer.

Using `std::atomic_store` and `std::atomic_load` ensures that the `InnerConfig` object is safely updated and accessed by multiple threads. Without these atomic operations, there could be race conditions and data inconsistencies when multiple threads access and modify the shared InnerConfig object concurrently.

The class `Config` receives a reference to the shared pointer and owns it only for a short period of the scope of a getter. This mechanism ensures that the class always provides the latest and greatest version of the config.

This solution comes with several important caveats.

- The values of configuration can be changed between calls to getters. If it is critical, need to extend the solution with an additional mutex for the whole `InnerConfig` object.
- The methods `std::atomic_store` and `std::atomic_load` are not guaranteed to be lock-free. It can be verified in runtime via `std::atomic_is_lock_free`. For example, on my platform, it returns `false`. If the performance is critical, you need to do benchmarks and decide if the standard implementation is good enough. For more information, please refer to [cppreference.com](https://en.cppreference.com/w/cpp/memory/shared_ptr/atomic).
- Once a shared pointer is passed to one of the atomic functions, it cannot be accessed non-atomically.
- The usage of `std::atomic_*` is deprecated in C++ 20 and replaced with [`std::atomic<std::shared_ptr>`](https://en.cppreference.com/w/cpp/memory/shared_ptr/atomic2) with the same caveats.

The complete source code is available in GitHub [repository](https://github.com/f-squirrel/shared_config).

Please share your ideas in the comments.

Update: Fixed a dangling reference in `Config::name` by returning by value, thanks to [dustyhome](https://www.reddit.com/r/cpp/comments/122udm0/comment/jdw5x11/?utm_source=share&utm_medium=web2x&context=3).

*Special thanks to [Sergey Pastukhov](https://www.linkedin.com/in/spastukhov/) for pinpointing a critical issue.*

Please share your thoughts on [Twitter](https://twitter.com/dbdanilov/status/1640052185781133313?s=20), [Reddit](https://www.reddit.com/r/cpp/comments/122udm0/using_shared_ptr_for_reloadable_config/) or [LinkedIn](https://www.linkedin.com/posts/ddanilov_using-sharedptr-for-reloadable-config-activity-7045819011908931584-a0vG?utm_source=share&utm_medium=member_desktop).
