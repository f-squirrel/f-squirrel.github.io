---
title: How signals are handled in a docker container
published: true
permalink: "/how-signals-are-handled-in-a-docker-container"
share-img: /img/docker-logo-696x364.png
share-description: "Deep dive into the mechanism of signal handling in docker containers"
tags: [docker, libc, linux, signals, sigsegv, sigabrt, c, kernel, init, abort]
readtime: true
comments: false
---

In my previous [post](/how-to-contain-a-crashed-container/), I provided insight on the importance of running docker with the
`--init` flag to ensure the proper exit code is returned when an application calls the `abort` function.

While researching on the subject, I stumbled upon a bug report on GitHub regarding this issue, for which a member of the community was only able to provide a workaround. My intrigue got the best of me and as a result, I am happy to share that I was able to update this [bug](https://github.com/moby/moby/issues/30593){:target="_blank"} report with my findings.

In this post, I am going to expand on these findings on the mechanism of signal handling in a docker container when it runs without this
flag.
<br>The first step is to take a look at how `abort` is
[implemented](https://github.com/bminor/glibc/blob/master/stdlib/abort.c#L46){:target="_blank"} in the GNU C Library.

The function initially unblocks the `SIGABRT` signal:

```c
 /* Unblock SIGABRT.  */
  if (stage == 0)
    {
      ++stage;
      __sigemptyset (&sigs);
      __sigaddset (&sigs, SIGABRT);
      __sigprocmask (SIG_UNBLOCK, &sigs, 0);
    }
```

Then it sends `SIGABRT` (line 13):

```c
 /* Send signal which possibly calls a user handler.  */
  if (stage == 1)
    {
      /* This stage is special: we must allow repeated calls of
  `abort' when a user defined handler for SIGABRT is installed.
  This is risky since the `raise' implementation might also
  fail but I don't see another possibility.  */
      int save_stage = stage;

      stage = 0;
      __libc_lock_unlock_recursive (lock);

      raise (SIGABRT);

      __libc_lock_lock_recursive (lock);
      stage = save_stage + 1;
    }
```

If the application does not have a user-defined signal handler (which my program in[the previous post](/how-to-contain-a-crashed-container/) did
not), then it should be caught by the default signal handler, i.e. the kernel.

Since my program's container was started without
the `--init` flag, the application had the PID 1 in the container's [PID
namespace](https://man7.org/linux/man-pages/man7/pid_namespaces.7.html){:target="_blank"}, and was treated as a standalone
["init"](https://man7.org/linux/man-pages/man1/init.1.html){:target="_blank"} process for this namespace.
**The Linux kernel handles signals differently for the init process
than it does for other processes. Signal handlers are not automatically registered for this process, meaning that
signals will not have effect by default.**
Hence, as in my program, the signal would not be handled and the function would continue running.

At this stage, `abort` assumes that the program has a user-defined handler which is malfunctioning, i.e. not killing the
process, thus `abort` replaces it with the default handler (line 6):

```c
 /* There was a handler installed.  Now remove it.  */
  if (stage == 2)
    {
      ++stage;
      memset (&act, '\0', sizeof (struct sigaction));
      act.sa_handler = SIG_DFL;
      __sigfillset (&act.sa_mask);
      act.sa_flags = 0;
      __sigaction (SIGABRT, &act, NULL);
    }
```

It then makes the last attempt to send the signal:

```c
/* Try again.  */
  if (stage == 3)
    {
      ++stage;
      raise (SIGABRT);
    }
```

As expected, sending the signal again does not help.

As a natural response to a consistently failing methodology, `abort` tries a different approach, where it attempts to execute a platform-specific command to terminate the
process:

```c
 /* Now try to abort using the system specific command.  */
  if (stage == 4)
    {
      ++stage;
      ABORT_INSTRUCTION;
    }
```

For the x86_64 architecture, the command is defined in
[glibc/sysdeps/x86_64/abort-instr.h](https://github.com/bminor/glibc/blob/master/sysdeps/x86_64/abort-instr.h){:target="_blank"}:

```
#define ABORT_INSTRUCTION asm ("hlt")
```

The instruction just pauses the CPU until the next external interrupt is fired, though it requires `ring 0` access which is available only for privileged software, such as the kernel. When a process
attempts to violate such permissions, the hardware triggers the general protection fault (GPF) interrupt, and a kernel is expected to kill the violating process. In Linux OS, the kernel’s general protection fault exception handler
([exc_general_protection](https://github.com/torvalds/linux/blob/master/arch/x86/kernel/traps.c#L525){:target="_blank"}) is called. The
handler checks if the violator is a userspace process and if so, it calls `force_sig(SIGSEGV);` which terminates the
process running in the docker, eventually setting its exit code to 139:

```c
if (user_mode(regs)) {
        tsk->thread.error_code = error_code;
        tsk->thread.trap_nr = X86_TRAP_GP;

        show_signal(tsk, SIGSEGV, "", desc, regs, error_code);
        force_sig(SIGSEGV);
        goto exit;
}
```

Once the application exits, docker sets its own exit code equal to the application’s.

As a demonstration of this behavior, I ran [dmesg](https://man7.org/linux/man-pages/man1/dmesg.1.html){:target="_blank"} right after the container
stopped in order to see the kernel logs:

<pre>
$ dmesg
[523555.291893] traps: app[109848] general protection fault ip:7ff4d9391a10 sp:7ffe992508a0 error:0 in libc-2.27.so[7ff4d9351000+1e7000]
</pre>
This message shows the process “app” with the PID 109848 (the program's PID outside of the
        container's PID namespace), the GPF error, and
Glibc (libc-2.27.so) that tried to execute the `HLT` instruction.

## Conclusion

Unless your application has user-defined signal handlers, it is strongly advised and encouraged to run a
container with the `--init` flag as a safety measure, as this tiny docker-implemented init process will enable default signal handling for
your application and reap zombie processes.

Please share your thoughts on [Twitter](https://twitter.com/dbdanilov/status/1345410839189315585?s=20).
