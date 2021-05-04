---
title: Guide to configuring core dumps in docker
published: true
share-description: "Detailed guide how to configure and collect core dumps in docker"
tags: [docker, debugging, gdb, coredump]
share-img: /img/docker-logo-696x364.png
readtime: true
permalink: "/how-to-configure-core-dump-in-docker-container"
---

I am going to describe how to enable and collect core dumps for applications running in a docker container.

> Disclaimer:
> This post is mostly a reminder for me.<br>

First of all, need to configure the host to save the dumps in a certain location.
The most generic way to do it is to set the core pattern. It consists of the path and additional information about the process that crashed. In this example I use the following pattern:
```plain
$ echo '/tmp/core.%e.%p' | sudo tee /proc/sys/kernel/core_pattern
```
* `/tmp` - directory where the files will be saved
* `core` - prefix of core's file name
* `%e` - process name
* `%p` - PID

For more details about the core pattern configuration, please refer to the [man page](https://man7.org/linux/man-pages/man5/core.5.html).

> Note: Another option to configure the host is to add the command above to a container’s entry point. Personally, I do not like this approach because the container has to run in a [privileged mode](https://docs.docker.com/engine/reference/run/#runtime-privilege-and-linux-capabilities) which theoretically may lead to a [security breach](https://www.trendmicro.com/en_us/research/19/l/why-running-a-privileged-container-in-docker-is-a-bad-idea.html).


Now, let us create a sample application that crashes immediately:
```cpp
#include <cstdlib>

void foo() {
    std::abort();
}

int main() {
    foo();
    return 0;
}
```

The Dockerfile is also very simple: it contains the build tools, GDB (we'll see why we need it later) and actual
compilation.
```docker
FROM ubuntu:18.04

# Install tools
RUN apt-get update \
    && apt-get -y install \
    gdb \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Build the application
COPY ./ /src/
WORKDIR /src/
RUN g++ main.cpp -o app

CMD ["/src/app"]
```

This is the time to run the application:
```plain
$ docker run \
        --init \
        --ulimit core=-1 \
        --mount type=bind,source=/tmp/,target=/tmp/ application:latest
```

* `init` - to ensure the [proper signal](/how-signals-are-handled-in-a-docker-container) handling in the container.
* `--ulimit core=-1` - set core dump size unlimited for the processes running in the container. Another option is to set
it globally at the host with [ulimit](https://docs.oracle.com/cd/E19683-01/816-0210/6m6nb7mo3/index.html).
* `--mount type=bind,source=/tmp/,target=/tmp/` - mount host's tmp to container’s tmp so that cores generated in
container are available after the container is stopped or deleted. Important note: the `source` path has to be the path
set in the core pattern!

After the application has crashed we can see core dumps in the host's `/tmp`:
```plain
$ ls /tmp/core*
/tmp/core.app.6
```

Despite the core dump file is available at the host, I recommend opening it in a container based on the same image,
where the application runs. It helps to make sure that all the dependencies are available for GDB.
```plain
$ docker run \
        -it \
        --mount type=bind,source=/tmp/,target=/tmp/ \
        application:latest \
        bash
```
The container is launched in the interactive mode (`-it`), the core location is mounted the same way as before but the
application running in the container is bash. Usually, docker images do not contain source code of applications, in this
case, need to mount also the source code' directory:
```plain
$ docker run \
        -it \
        --mount type=bind,source=/tmp/,target=/tmp/ \
        --mount type=bind,source=<path to source at host>,target=/src/ \
        application:latest \
        bash
```

Eventually, inside of the container, we run GDB:
```plain
root@1679288711ff:/src# gdb app /tmp/core.app.6
```

Once GDB is ready, run `bt` to see backtrace:
```plain
(gdb) bt
#0  __GI_raise (sig=sig@entry=6) at ../sysdeps/unix/sysv/linux/raise.c:51
#1  0x00007f263f378921 in __GI_abort () at abort.c:79
#2  0x000055f9a9d16653 in foo() ()
#3  0x000055f9a9d1665c in main ()
```

As you can see, the backtrace reflects the place where the abort function is called.
