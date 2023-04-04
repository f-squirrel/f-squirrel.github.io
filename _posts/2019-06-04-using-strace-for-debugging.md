---
title: Using strace for debugging
published: true
permalink: "/using-strace-for-debugging/"
share-img: /img/Strace_logo-156x300.png
tags: [strace, debugging, linux]
readtime: true
comments: false
---

Debugging is an important part of software engineering and every
developer has a few debugging tools in his toolbelt.
Usually, I use logs and a debugger and that's enough for my everyday work.
But lately, I had to work with an open source SIP client called baresip.
Baresip is a very stable open-source SIP client with an excellent design supporting plugins.

After a while, I discovered that sometimes baresip was unable to start playing
audio for outgoing calls because of system error "Device or resource is busy".
I noticed it did not happen when the client was answering an incoming call.
My first impression was that baresip tried to open audio for playing incoming audio twice.
I tried to find this place in the code but since I was not familiar with the code I did not
manage to do that. And then I thought of `strace`.

<p align="center">
  <img src="/img/Strace_logo-156x300.png" title="Strace logo">
</p>

`strace` is a userspace tool that prints all system calls of a given process.
You may start a process inside strace or you can attach it to a running process.
I ran baresip with strace: `strace -o strace.log baresip -e 7100`, where:

* -o filename Write the trace output to the file filename rather than to stderr.
* strace.log is the output log file
* baresip is the process under investigation
* -e is the command line argument saying to baresip to dial the number 7100.

And then I looked for "Device or resource is busy" in the log file.

![strace](/img/strace_problematic_open_call.png)

And as we can see it happens in line 1308. By the way, vim has builtin syntax highlight for strace logs.

So I looked for `open("/dev/snd/pcmC0D0p", O_RDWR|O_NONBLOCK|O_LARGEFILE|O_CLOEXEC)`.

![strace](/img/first_open_call_0.png)

As you see the open returns file descriptor "14". So let's look whether baresip closes the file descriptor:

![strace](/img/close_14.png)

Next open call:

![strace](/img/second_open_call.png)

And the corresponding close call in line 1484:

![strace](/img/second_close_call.png)

And if you still follow me here is the next open call:

![strace](/img/third_open_call.png)

So as you may see the next open function call happens in line 1308 while close is called in line 1484.
Meaning baresip opens the audio device before closing it.
Looks like we found out the problematic function calls!

As far as I understand baresip opens the audio device to play
incoming audio but why does it open it before? So let's take a look at what happens around the second open call. In line 1120 we see that baresip opens and then reads ringback.wav file which is
a ringback tone, the audible ringing that is heard by the calling party after dialing:

![strace](/img/open_ringback_file_epoll_wait.png)

And after that, it obviously opens the audio device in order to play the file.
And while the audio device is playing the ringtone another thread receives a response
from the callee and opens the same audio device in order to play the incoming audio.
And then the problem happens.

This is how with very limited knowledge about the application I managed to find out the root cause.
I didn't fix the issue because it required significant changes in baresip' design but I
found a quick workaround by disabling the ringback in a config file.
