---
title: Docker run/exec and carriage return
published: true
permalink: "/docker-run-exec-and-carriage-return/"
share-img: /img/docker-logo-696x364.png
tags: [docker]
readtime: true
comments: false
---


Recently, I was writing a script whose function was to retrieve and parse a list of processes that were running in a docker container.
The script was based on another script that did the same by `ssh`-ing to a remote server.
However, my `grep/sed/awk` command set did not work despite the printed output in the terminal looked identical to the one from the original script.

![docker logo](/img/docker-logo-696x364.png)

This is a simplified version of the command:

```sh
$docker exec -it container_name echo "Hello”
Hello
```

As you may see, the output looks very normal. I decided to print the output with C-styled escaped characters using the
[od](https://man7.org/linux/man-pages/man1/od.1.html){:target="_blank"} utility:

```sh
$docker exec -it container_name echo "Hello" | od -c
0000000    H   e   l   l   o  \r  \n
0000007
```

You may notice that the output contains the carriage return character(`\r`)!
In contrast, the `ssh` output does not:

```sh
$ssh user@remote_server "echo Hello" | od -c
0000000    H   e   l   l   o  \n
0000006
```

In order to understand where the difference comes from, we need to take a look at the parameters of `docker exec -it`.
By default, docker containers have only `STDOUT` attached, therefore a container’s output is printed to the host’s terminal.
If a user needs to send an input to the container, the container has to have `STDIN` open. In order to do so, you need to run `docker exec -i` or `docker exec --interactive=true`.
It is likely that most applications don’t need more parameters except those that use the `TTY` features, such as text coloring or `curses`. To provide them with this ability, the container has to run with `-t` or `--tty=true`.
A good example of such an application is `vim`. The only way to use `vim` inside a container is to run/execute the container with `-it`.

_It seems that the default behavior of tty is to add the carriage return._

In my case, I did not actually need `STDIN` nor `TTY`, so I ran the container without `-it`:

```sh
$docker exec container_name echo "Hello" | od -c
0000000    H   e   l   l   o  \n
0000006
```

The result is **no carriage return**!

In case you do need `TTY` but don’t want the carriage return, there are a few options:
Delete `\r` from the container’s output using [tr](https://linux.die.net/man/1/tr){:target="_blank"}:

```sh
$docker exec -it container_name echo "Hello" | tr -d '\r' | od -c
0000000    H   e   l   l   o  \n
0000006
```

Configure the container’s `TTY` to translate newline to carriage return-newline:

```sh
$docker exec -it container_name /bin/bash -c "stty -onlcr && echo 'Hello'" | od -c
0000000    H   e   l   l   o  \n
0000006
```

Note: all of the above applies to `docker run` as well.

Please share your thoughts on [LinkedIn](https://www.linkedin.com/posts/ddanilov_docker-activity-6669324289147248640-mD22?utm_source=share&utm_medium=member_desktop).
