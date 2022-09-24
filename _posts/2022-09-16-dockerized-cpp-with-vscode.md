---
title: VS Code with dockerized build environments for C/C++ projects
published: true
tags: [c, cpp, docker, make, build, vscode]
share-img: /img/docker-logo-696x364.png
readtime: true
permalink: "/dockerized-cpp-build-with-vscode"
share-description: How to configure VS Code to work with dockerized builds
---

After I posted [Dockerized build environments for C/C++ projects](/dockerized-cpp-build) a few people asked me both online and offline about how I use IDE/text editor in this setup.

First of all, I would like to define the problem.
From my point of view, there are the following problems: autocompletion, build, and debugging. Since I am a supporter of the [Unix philosophy](https://en.wikipedia.org/wiki/Unix_philosophy),
<!-- probably need to rephrase it --> I build and debug not via an IDE/text editor but directly from the terminal. I truly believe that tools like `make` and `gdb` are more powerful than any GUI alternative. In this article, I am going to talk mostly about rich language support: autocompletion and syntax highlighting.
It does not mean that the rest is not achievable via my approach, moreover, I am almost certain it is, but this is not the focus.

There are next types of autocompletion mechanisms:

* **Index/parser** based is quite simple: there is an application that parses the code and indexes it, later when autocompletion is triggered, the application looks up for a symbol in the index database. Probably, the most famous implementation is [ctags](https://en.wikipedia.org/wiki/Ctags) and its descendants. The major problem with such tools is that it is extremely hard to write a good parser for C++ which often makes the index database and cross-referencing between files in a project inaccurate.
*Index-based tools are agnostic to dockerized builds*.

* [**Tree-sitter**](https://tree-sitter.github.io/tree-sitter/) is a lightweight parser that builds an abstract syntax tree ([AST]([AST](https://en.wikipedia.org/wiki/Abstract_syntax_tree))) for each source file. The tree is used to collect data about text objects in source files, it is stored in a database or a memory and used to provide information for auto-completion. Unfortunately, its autocompletion capabilities are quite limited at the moment, mostly because it has no information about the build system and parses every file individually. However, it is widely used as a data provider for extended syntax highlighting.
*This mechanism is also insensitive to dockerized builds*.

* **Compiler-based** is more complicated: it utilizes the compiler's original parser to build an abstract syntax tree ([AST](https://en.wikipedia.org/wiki/Abstract_syntax_tree)), which is used for indexing.
As a result, the autocompletion metadata is collected by the best language parser in the world, the compiler.
Usually, this kind of tool provides the most accurate information about the source for auto-completion.
*Almost always are affected by the dockerized builds*.
Also, it is often the heaviest option.

* **Hybrid** is a mixture of the above.

Since the accuracy of autocompletion is extremely important for me, I always try to use compiler-based tools.

## Clangd

The first and meanwhile the only compiler-based autocompletion parser for C++ is [clangd](https://clangd.llvm.org/); it is based on the [Clang](https://clang.llvm.org/) C++ compiler and part of the [LLVM](https://llvm.org/) project. Once the `clangd` server is launched, it looks for a file `compile_commands.json`. It contains a list of files in the project together with the compiler flags. To instruct CMake to generate this file, add the following to the top-level `CMakeLists.txt` file:

```cmake
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
```

*For information about other build systems and customized configurations, please refer to the official [documentation](https://clangd.llvm.org/installation#project-setup).*

Usually, I add a symlink to this file from the top-level folder in the source tree:

```sh
cd Project/
ln -s ./build/compile_commands.json compile_commands.json
ls -l compile_commands.json
lrwxrwxrwx 1 dima dima 36 Aug  2 17:13 compile_commands.json -> ./build/compile_commands.json
```

 For the example [project](https://github.com/f-squirrel/dockerized_cpp_build_example) I have created for the post about [Dockerized build environments for C/C++ projects](/dockerized-cpp-build), the generated `compile_commands.json` looks like the following:

```json
[
{
  "directory": "/example/build",
  "command": "/usr/bin/g++  -DBOOST_ALL_NO_LIB -I/example -isystem /usr/local/include    -Wall -Wbuiltin-macro-redefined -pedantic -Werror -std=c++1z -o CMakeFiles/a.out.dir/main.cpp.o -c /example/main.cpp",
  "file": "/example/main.cpp"
}
]
```

The JSON contains three entries per each file:

* Path to the directory where `compile_commands.json` files are located
* Compile command, including defines, include paths, C++ standard used, flags, etc
* Path to the C++ file

Note that all the paths are *absolute*, which means that `clangd` has to run on the identical filesystem as the build system. This becomes a problem when building in docker: the `clangd` server runs on the host while the absolute path to source files and includes are from the docker container's filesystem. Don't worry, we will handle it later in this post.

After the file is loaded, the server parses each file, build's its AST, collects metadata about variables and functions usage, `#defines` and `#if-defs` related to this file and others, and stores it in the `.cache` folder. Note that this folder is created in the same directory where `compile_commands.json` is located.
The parsed data is stored per cpp file in some `*.idx` format:

```sh
-rw-r--r-- 1 dima dima 7.6K Sep 15 11:47 robot.cpp.5254FE304AF08338.idx
-rw-r--r-- 1 dima dima  26K Sep 18 09:13 spaceship.h.78C478E3644A6BF2.idx
```

Fortunately, clangd watches the files and updates the index in the background.

## VS Code with Clangd

The first time, I used it, was via an amazing Vim plugin [YouCompleteMe](https://github.com/ycm-core/YouCompleteMe), later I switched to [Neovim](/the-switch-from-vim/) and started using `clangd` as an [LSP](https://microsoft.github.io/language-server-protocol/) server. A few months ago, I have switched to a new setup VS Code with [VSCode Neovim](https://github.com/vscode-neovim/vscode-neovim) plugin. Yes, I admit the addiction to Vim motions.

So, to enable `clangd` in VS Code, first of all, need to install the official LLVM [extension](https://marketplace.visualstudio.com/items?itemName=llvm-vs-code-extensions.vscode-clangd). After it is installed it will propose to install the latest `clangd` server, if you don't have it already installed, I suggest agreeing. While for natively build projects, it is enough, the dockerized builds require an extra step.

## VS Code with Docker Support

VS Code provides Docker support via [Remote - Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) extension. It lets using a Docker container as a full-featured development environment and allows opening any folder inside (or mounted into) a container and taking advantage of Visual Studio Code's full feature set.

A file `.devcontainer.json` in the top-level directory of the project tells VS Code to re-load it in a Docker container. For our example project, it will be like the following:

```json
{
 "image": "example/example_build:latest",
 "runArgs": [
  "--cap-add=SYS_PTRACE",
  "--security-opt",
  "seccomp=unconfined"
 ],
 "customizations": {
  "vscode": {
   "extensions": [
    "llvm-vs-code-extensions.vscode-clangd",
   ]
  }
 },
 "workspaceMount": "source=${localWorkspaceFolder},target=/example,type=bind",
 "workspaceFolder": "/example",
 "onCreateCommand": "apt update && apt install -y git && git config --global --add safe.directory /example"
}
```

where:

* `"image"` - the image we use for building the project
* `"runArgs"` - represents the flags provided to `docker run` for launching the container for auto-completion. In this example, I added the basic flags required for debugging in Docker.
* `"extensions"` - a list of VS Code extensions to be installed in the container. I have added only `clangd` at the moment but more will be added soon.
* `"workspaceMount"` - mounting point, in our case we mount the current directory to the directory `example` in the container. The very same way we did in the build container.
* `"onCreateCommand"` - this one is a bit ugly: the build container comes without Git, so I had to install it every time, the auto-completion container is created. I believe, there is a cleaner way to do it but it worked for me just fine.

After the Docker extension is installed and the `.devcontainer.json` file is placed in the repository, VS Code proposes to re-open the project in the container:

![Open in container](/img/vscode_reopen_in_container_prompt.png)
  
Once a user agrees, VS Code launches a Docker container based on the `"image"` and adds there the extensions specified in `"extensions"`. Since the base image does not contain `clangd`, VS Code will ask to install it, I recommend agreeing, the same as for the native builds.

![Install Clangd](/img/vscode_asks_to_install_clangd.png)

Once installed and reloaded, VS Code launches the `clangd` server, we can see it because the source tree contains the `.cache` directory with the index database and the auto-completion works like a charm.

![Auto-completion with the cache folder](/img/vscode_with_clangd_in_docker_autocompletion.png)

Additionally, `clangd` provides error and warning messages based on compiler diagnostics and clang-tidy configuration (if exists):

![Compiler diagnostics](/img/vscode_diagnostics_1.png)

And fix suggestions for the cases when the compiler can help:

![Fix suggestion](/img/vscode_code_suggetions.png)

The last but important feature is the support of format based on the `.clang-format` file.

## Further Improvements

After the important things are set up, the reader may add other useful extensions to the `.devcontainer.json`. It might be important because once launched in Docker, all the interaction with source code is done from within a container, i.e., if the reader uses any plugins for editing file types other than C++ within their project, it is recommended to add the corresponding plugins to the `.devcontainer.json` file.

For example, CMake files are not highlighted properly and to improve it, need to install the CMake plugin in the container, as shown in the picture below:

![Install CMake in Container](/img/vscode_without_cmake_in_docker.png)

After the plugin is installed, I suggest adding it to the `.devcontainer.json` file either manually or via the UI so that the next time the container is launched, the extension will be installed automatically:

![Add to devcontainer](/img/vscode_add_todevcontainer.png)

Besides those plugins, my setup usually includes various plugins depending on the project: Python plugin(s), Grammarly (checks grammar in text and markdown files), Git Lens, markdown linter, and others.

Some users might find the [CodeLLDB](https://marketplace.visualstudio.com/items?itemName=vadimcn.vscode-lldb) extension useful, it provides a native debugger powered by LLDB, to debug C++, Rust, and other compiled languages.

## Additional information

For the full information about the configurations available via the `.devcontainer.json` file, please refer to the official [page](https://code.visualstudio.com/docs/remote/containers#_create-a-devcontainerjson-file).

As usual, the example [project](https://github.com/f-squirrel/dockerized_cpp_build_example) at Github is updated with the docker configuration described in this post.
