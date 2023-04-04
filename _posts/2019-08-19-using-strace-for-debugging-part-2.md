---
title: Using strace for debugging, part 2
published: true
permalink: "/using-strace-for-debugging-part-2/"
share-img: /img/Strace_logo-156x300.png
tags: [strace, debugging, linux]
readtime: true
comments: false
---

This is the second post about debugging with strace.

In our solution, we have a process that runs another process.
The parent communicates with the child process via TCP socket.
In order to know whether the child is ready for communication the parent reads
it's stdout and looks for the readiness indication string, e.g. "I'm ready".
Everything worked fine but suddenly my colleague and I started observing a strange behavior:
the child process was stuck after a while.

So I ran `strace -p 1666`, where 1666 is the child' id.
strace output:
<pre>
$sudo strace -p 25485
strace: Process 25485 attached
write(1, "Some important information...."..., 101
</pre>

The child process was trying to write something to
stdout(1 is the file descriptor of stdout) but was unable to do it!
I tried to reproduce the issue by running the process manually.
And to our surprise the issue did not reproduce! So I started looking on the way our application was starting the process:

```python
cmd = ['./child.py']
handle = subprocess.Popen( cmd, stdout=subprocess.PIPE,
                           stderr = subprocess.STDOUT,
                           close_fds=True, shell=True)
wait_for_child_readiness(handle)
#do_some_real_work()
handle.wait()
```

It looked quite normal: Start a process and then forward stdout to stderr
and pipe both to the parent process. And then the parent process reads the output:

```python
def wait_for_child_readiness(proc_handle):
        last_stdout = ""
        while "CHILD IS READY" not in last_stdout :
                last_stdout = proc_handle.stdout.read(512)
                sys.stdout.write(last_stdout)
                sys.stdout.flush()
        sys.stdout.write("\nParent detected child's readiness!\n")
```

Then we read the output until we find the indication string.
But wait! The child process continues writing to stdout/stderr and pipe continues sending
the output to the parent process. But the parent process does not read it.
And then pipe overflow happens and the child process cannot write anything to stdout!

The solution was to continue reading the output of the child process.
Hence once you start reading child process output don't stop doing that!;)

P.S. The way to know the pipe size on Linux is to check:
<pre>
$cat /proc/sys/fs/pipe-max-size
1048576
</pre>
The default value for this file is 1048576 (1 MiB)

[http://man7.org/linux/man-pages/man7/pipe.7.html](http://man7.org/linux/man-pages/man7/pipe.7.html){:target="_blank"}
