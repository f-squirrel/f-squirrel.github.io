---
title: Configuring core dumps in docker
published: true
share-description: "Detailed guide on how to configure and collect core dumps in docker"
tags: [docker, debugging, gdb, coredump, linux]
share-img: /img/docker-logo-696x364.png
readtime: true
permalink: "/how-to-configure-core-dump-in-docker-container"
---

The purpose of this post is to provide general guidance on enabling and collecting core dumps for applications running
in a docker container.

> Disclaimer:<br>
> This post is mostly for quick reference for myself and any interested parties.<br>

First of all, the **host** needs to be configured to save the dumps in a certain location.
The generic method is to set the system's core pattern, which consists of a path and potentially useful information
about a process that crashed. In the below example, I use the following pattern:

```plain
echo '/tmp/core.%e.%p' | sudo tee /proc/sys/kernel/core_pattern
```

* `/tmp` - directory where the files will be saved
* `core` - prefix of core's filename
* `%e` - process name
* `%p` - PID

For more details about core pattern configuration, please refer to the [man page](https://man7.org/linux/man-pages/man5/core.5.html){:target="_blank"}.

> Note:<br>
> Another option is to configure the host via the container’s
> CMD or ENTRYPOINT. Personally, I do not like this approach because the container has to run in [privileged
> mode](https://docs.docker.com/engine/reference/run/#runtime-privilege-and-linux-capabilities){:target="_blank"} which theoretically may
> lead to a [security
> breach](https://www.trendmicro.com/en_us/research/19/l/why-running-a-privileged-container-in-docker-is-a-bad-idea.html){:target="_blank"}.

The following sample application crashes immediately:

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

The Dockerfile presented below will be used to build the image for our sample application: it installs
the build-essentials, GDB (needed for future use), copies source code, and builds the application.

```docker
FROM ubuntu:18.04

# Install tools
RUN apt-get update \
    && apt-get -y install \
    build-essential \
    gdb \
    && rm -rf /var/lib/apt/lists/*

# Build the application
COPY ./ /src/
WORKDIR /src/
RUN g++ main.cpp -o app

CMD ["/src/app"]
```

Now let us run the application:

```plain
$ docker run \
        --init \
        --ulimit core=-1 \
        --mount type=bind,source=/tmp/,target=/tmp/ application:latest
```

* `--init` - to ensure [proper signal](/how-signals-are-handled-in-a-docker-container){:target="_blank"} handling in the container.
* `--ulimit core=-1` - set core dump size to unlimited for the processes running in the container.
* `--mount type=bind,source=/tmp/,target=/tmp/` - mount host's tmp directory to the container’s tmp so that cores
generated in the container remain available after the container is stopped or deleted.<br>
*It is important to note that the `source` path has to be the path
set in the core pattern!*

After the application has crashed, the core dumps can be found in the host's `/tmp` directory:

```plain
$ ls /tmp/core*
/tmp/core.app.6
```

Despite the core dump file is available in the host file system, I recommend opening it in a container created from the
same image where the application was built. It also helps to make sure that all dependencies are available for GDB.

```plain
$ docker run \
        -it \
        --mount type=bind,source=/tmp/,target=/tmp/ \
        application:latest \
        bash
```

* `-it` - launch the container in interactive mode.
* `--mount type=bind,source=/tmp/,target=/tmp/`- the core location is mounted the same way as before.
* `bash` - this time, the application running in the container is bash.

Usually, docker images do not contain source code and in this
case, the source code directory needs to be mounted as well:

```plain
$ docker run \
        -it \
        --mount type=bind,source=/tmp/,target=/tmp/ \
        --mount type=bind,source=<path to source at host>,target=/src/ \
        application:latest \
        bash
```

Eventually, inside of the container, run GDB:

```plain
root@1679288711ff:/src# gdb app /tmp/core.app.6
```

Once GDB is ready, run `bt` to view the backtrace:

```plain
(gdb) bt
#0  __GI_raise (sig=sig@entry=6) at ../sysdeps/unix/sysv/linux/raise.c:51
#1  0x00007f263f378921 in __GI_abort () at abort.c:79
#2  0x000055f9a9d16653 in foo() ()
#3  0x000055f9a9d1665c in main ()
```

The backtrace will conveniently lead you straight to the source of the issue which caused the application to crash.

P.S. docker-compose supports all flags that have been referenced in this post.

Please share your thoughts on [Twitter](https://twitter.com/dbdanilov/status/1391843816156631050?s=20) or [LinkedIn](https://www.linkedin.com/posts/ddanilov_configuring-core-dumps-in-docker-dmitry-activity-6797608842315214848-xjba?utm_source=share&utm_medium=member_desktop).
