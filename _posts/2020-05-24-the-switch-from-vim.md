---
title: The switch from Vim to Neovim
published: true
permalink: "/the-switch-from-vim/"
share-img: /img/Vimlogo.svg.png
tags: [vim, gvim, macvim, nvim, neovim, neovim-qt]
readtime: true
---

For the past five years, my go-to text editors have been Vim and [gVim](https://en.wikipedia.org/wiki/Vim_(text_editor)#Interface){:target="_blank"}/[MacVim](https://macvim-dev.github.io/macvim/){:target="_blank"}.
Currently, I work on macOS with Ubuntu, as a target OS, running on the local VMware Fusion virtual machine.

Personally, I prefer to use the macOS GUI over Ubuntu and do not enjoy constantly switching between windows,
so I used to connect to the machine with `ssh -XY user@server` and run gVim there.
Since macOS supports [X Window System](https://www.xquartz.org/){:target="_blank"}, I was able to open the gVim window in macOS as a “native” application. However, when I had to edit a file locally on macOS, naturally I used MacVim.

Over time, it became increasingly inconvenient because the behavior and appearance of gVim and MacVim had minor differences.
Additionally, the X Window System simply does not fit the macOS ecosystem well.

## Visual Studio Code ##

I started to search for efficient alternatives and almost ended up switching to [Visual Studio Code](https://code.visualstudio.com/){:target="_blank"}.
Don’t get me wrong - VS Code is an awesome text editor with features that a Vim user can only dream of, but I got used to my Vim shortcuts and plugins. I know there is Vim support in VS Code, but it is not the same.

## Neovim ##

Eventually, I discovered [Neovim](https://neovim.io/){:target="_blank"}.

> Neovim is a refactor, and sometimes redactor, in the tradition of Vim (which itself derives from Stevie). It is not a rewrite but a continuation and extension of Vim.
>
> Nvim always includes ALL features, in contrast to Vim (which ships with
various combinations of 100+ optional features). Think of it as a leaner
version of Vim's "HUGE" build. This reduces surface area for bugs, and
removes a common source of confusion and friction for users.

Once Neovim is installed, it behaves the same way as Vim and supports all of its plugins.
Basically, it is a drop-in replacement of Vim.

Neovim also supports remote plugins that communicate via [msgpack-rpc](https://msgpack.org/){:target="_blank"}.
The RPC messages can be sent through various channels, such as Unix socket, TCP socket, or stdin/stdout.

Neovim GUI clients are implemented as remote plugins and most of them communicate through stdin/stdout.
Each GUI client launches `nvim` process and sends it commands in `msgpack` format via stdin, and `nvim`
replies back via stdout with information on how to redraw the screen.

## Neovim + Neovim-Qt ##

[Neovim-Qt](https://github.com/equalsraf/neovim-qt){:target="_blank"} is a compact Neovim GUI client written in C++ with Qt5.
If I need to edit a file locally, I open Neovim-Qt, which works as described above.
Additionally, Neovim-Qt is able to connect to a Neovim instance, operating in server mode.
In order to edit files on the virtual machine, I launch a `nvim` process within the virtual environment, enabling listening on the given IP/port:

```sh
ssh user@server
nvim --listen <ip>:<port> \
      --headless #headless means "don't start a user interface"
```

and then start Neovim-Qt with the following parameters on macOS:

```sh
nvim-qt --server <ip>:<port>
```

Running all of these commands manually every time I need to open a text editor is quite tedious.
Therefore, I decided to automate the process with a simple [script](https://github.com/f-squirrel/scripts/blob/master/utils/run_nvim_remotely.zsh){:target="_blank"} that provides the alias [`rgvim`](https://github.com/f-squirrel/scripts/blob/master/utils/run_nvim_remotely.zsh#L78){:target="_blank"}, where `r` stands for “remote”.

Finally, I can use a single GUI for both operating systems!

## Screenshots ##

Local macOS environment:

![Local macOS](/img/neovim-qt-local.png)

Connected to a remote instance:

![Remote connection](/img/neovim-qt-remote.png)

Please share your thoughts on [LinkedIn](https://www.linkedin.com/posts/ddanilov_vim-neovim-activity-6670601427926216705-s0xD?utm_source=share&utm_medium=member_desktop).
