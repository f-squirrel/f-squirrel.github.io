---
title: Docker run/exec and carriage return
published: true
permalink: "/docker-run-exec-and-carriage-return/"
share-img: /img/docker-logo-696x364.png
tags: [docker]
readtime: true
---


Recently, I was writing a script whose function was to retrieve and parse a list of processes that were running in a docker container.
The script was based on another script that did the same by `ssh`-ing to a remote server.
However, my `grep/sed/awk` command set did not work despite the printed output in the terminal looked identical to the one from the original script.

<p align="center">
  <img src="/img/docker-logo-696x364.png">
</p>

This is a simplified version of the command:
<pre>
$docker exec -it container_name echo "Hello”
Hello
</pre>
As you may see, the output looks very normal. I decided to print the output with C-styled escaped characters using the
[od](https://man7.org/linux/man-pages/man1/od.1.html) utility:
<pre>
$docker exec -it container_name echo "Hello" | od -c
0000000    H   e   l   l   o  \r  \n
0000007
</pre>
You may notice that the output contains the carriage return character(`\r`)!
<br>In contrast, the `ssh` output does not:
<pre>
$ssh user@remote_server "echo Hello" | od -c
0000000    H   e   l   l   o  \n
0000006
</pre>

In order to understand where the difference comes from, we need to take a look at the parameters of `docker exec -it`.
<br>By default, docker containers have only `STDOUT` attached, therefore a container’s output is printed to the host’s terminal.
If a user needs to send an input to the container, the container has to have `STDIN` open. In order to do so, you need to run `docker exec -i` or `docker exec --interactive=true`.
It is likely that most applications don’t need more parameters except those that use the `TTY` features, such as text coloring or `curses`. To provide them with this ability, the container has to run with `-t` or `--tty=true`.
A good example of such an application is `vim`. The only way to use `vim` inside a container is to run/execute the container with `-it`.
<br><span style="background-color: #FFFF00">It seems that the default behavior of tty is to add the carriage return.</span>

In my case, I did not actually need `STDIN` nor `TTY`, so I ran the container without `-it`:
<pre>
$docker exec container_name echo "Hello" | od -c
0000000    H   e   l   l   o  \n
0000006
</pre>
The result is **no carriage return**!

In case you do need `TTY` but don’t want the carriage return, there are a few options:
Delete `\r` from the container’s output using [tr](https://linux.die.net/man/1/tr):
<pre>
$docker exec -it container_name echo "Hello" | <span style="background-color: #00CC66">tr -d '\r'</span> | od -c
0000000    H   e   l   l   o  \n
0000006
</pre>
Configure the container’s `TTY` to translate newline to carriage return-newline:
<pre>
$docker exec -it container_name /bin/bash -c "<span style="background-color: #00CC66">stty -onlcr</span> && echo 'Hello'" | od -c
0000000    H   e   l   l   o  \n
0000006
</pre>

Note: all of the above applies to `docker run` as well.
