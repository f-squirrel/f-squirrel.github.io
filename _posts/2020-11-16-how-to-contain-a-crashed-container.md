---
title: Exit codes in docker when a program aborts
published: true
permalink: "/how-to-contain-a-crashed-container/"
share-img: /img/docker-logo-696x364.png
share-description: "Tips on how to ensure correct exit codes in docker containers"
tags: [
    docker,
    signals,
    sigsegv,
    sigabort,
    init,
    linux,
    c,
    debugging,
    docker-compose,
]
readtime: true
comments: false
---

In an earlier post of mine [“Let it explode!”](/let-it-explode/), we explored "handling" certain exceptions via
premature termination.  Now, it is time to solve the mystery of process termination in Docker. <br>

Nowadays, Docker containers have become the de facto standard of shipping applications.  While running docker
containers, I noticed an unexpected exit code when a contained application crashed.<br> Out of curiosity and while I had
a few minutes left for Futurama to finish downloading, I created a sample application in order to investigate this
sorcery:

```cpp
#include <cstdlib>

int main() {
    std::abort();
    return 0;
}
```

Dockerfile:

```docker
FROM ubuntu:18.04

# Install tools
RUN apt-get update \
    && apt-get -y install \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Build the application
COPY ./ /src/
WORKDIR /src/
RUN g++ main.cpp -o app

WORKDIR /
CMD ["/src/app"]
```

The result once executed:
<pre>
$ docker build -f ./Dockerfile -t sigabort_test:latest .
$ docker run --name test sigabort_test:latest ; echo $?
<span style="background-color: #FFFF00">139</span>
</pre>
In Unix-like operating systems, if a process is terminated with a signal, the exit code is the result of `128` + the
signal number[[1]](https://tldp.org/LDP/abs/html/exitcodes.html){:target="_blank"}. In the example above, the exit code is `139 = 128 +
11`, where `11` represents `SIGSEGV` (segmentation fault) instead of `134 = 128 + 6` which is `SIGABRT` (abort).

Most users are generally only interested in knowing whether the application works, which they check and confirm when the
exit code is `0`.  For debugging purposes, however, it is very useful to understand the cause of an unexpected
termination.

During my research, I was able to find an open [issue](https://github.com/moby/moby/issues/30593){:target="_blank"} on the `SIGABRT`
dilemma and a comment with the following workaround using bash:

```docker
CMD ["bash", "-c", "/src/app ; exit $(echo $?)"]
```

Now, the container returns the correct exit code:
<pre>
$ docker run --name test sigabort_test:latest ; echo $?
bash: line 1:     6 Aborted                 /src/app
<span style="background-color: #00FF00">134</span>
</pre>

Another way, which can be considered the correct method, is to solve the problem by adding the `--init`
[flag](https://docs.docker.com/engine/reference/run/#specify-an-init-process){:target="_blank"}. This indicates that an [init
process](https://en.wikipedia.org/wiki/Init) should be used as the PID 1 in the container. Specifying the init process
ensures that the usual responsibilities of an init system, such as reaping zombie processes and default signal handling,
are performed inside of the created container.

The following is the result of running the container with the `--init` flag and the original Dockerfile command `CMD
["/src/app"]`:

<pre>
$ docker run --init --name test sigabort_test:latest ; echo $?
<span style="background-color: #00FF00">134</span>
</pre>

P.S. `docker-compose` also [supports](https://docs.docker.com/compose/compose-file/compose-file-v2/#init){:target="_blank"} the init flag
from version 2.4 and onward.

_For a detailed explanation on signal handling in docker, please have a look at
my next article [How signals are handled in a docker
container](/how-signals-are-handled-in-a-docker-container)._

Please share your thoughts on [Twitter](https://twitter.com/dbdanilov/status/1328270769563062273?s=20) or [LinkedIn](https://www.linkedin.com/posts/ddanilov_docker-linux-signals-activity-6734035710958944256-nhSf?utm_source=share&utm_medium=member_desktop).