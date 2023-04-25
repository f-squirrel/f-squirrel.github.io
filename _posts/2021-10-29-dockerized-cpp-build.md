---
title: Dockerized build environments for C/C++ projects
published: true
tags: [c, cpp, docker, make, build]
share-img: /img/docker-logo-696x364.png
readtime: true
permalink: "/dockerized-cpp-build"
share-description: The complete guide on implementing Docker-based builds for C/C++ projects
---

In this post, I will share how to create a docker-based build environment for C and C++ projects targeted for Linux.

## Common problems of building projects “natively” on a workstation

First of all, let us discuss why building C/C++ projects directly on a workstation may become a problem. C++ does not provide a built-in dependency management mechanism and as a result, third parties are added using a mix of techniques: installing from Linux distro’s repositories (e.g. apt-get) or via “make install”, adding 3rd parties as git submodules and building them within the source tree, or using a half-baked solution like Conan.
<br>Unfortunately, all of them have certain disadvantages:

* Installing dependencies on a dev machine makes the environment dirty and rarely the same as CI/CD or production, especially after updating the 3rd parties.
* Adding 3rd parties as git submodules requires building them within the project’s source tree. In cases when a 3rd party is heavy (boost, Protobuf, Thrift, etc), this approach may slow down the build so significantly that developers become reluctant to clean a build directory or switch between branches.
* Solutions like Conan often lack the right version of a certain dependency and adding it requires writing code in Python, which from my point of view is a bit too much.

## A single, isolated, and reproducible build environment

The preferred solution to the problems mentioned above is to create a docker image with preinstalled dependencies and tools such as compilers, debugger, etc, and build the project inside a container based on this image.

This image will be the base of a **single** build environment used by developers on their workstations and CI/CD servers, i.e. no more “it works on my machine but fails at CI!”.

Since the build runs inside of a container, it is not affected by any environment variables, tools, or settings specific to a developer’s local environment, which means that the environment becomes **isolated**.

Ideally, docker images are properly tagged with some meaningful version names; it allows users to jump between environments by pulling the right image from the registry. Even if the image has been removed from the registry, docker images, as well known, are built from Dockerfiles which in their turn are part of git repositories. Thus, it is always possible to rebuild the image from an old Dockerfile. All this makes the dockerized build environment **reproducible**.

## Creating the build image

Let us create a simple application and build it in a container. The application will print its size with `boost::filesystem`. I have chosen boost to show an example of using docker with a “heavy” third party.

```cpp
#include <boost/filesystem/operations.hpp>
#include <iostream>

int main(int argc, char *argv[]) {
    std::cout << "The size of " << boost::filesystem::absolute(argv[0])
              << " is " << boost::filesystem::file_size(argv[0]) << '\n';
    return 0;
}
```

The CMake file is quite simple:

```cmake
cmake_minimum_required(VERSION 3.10.2)

project(a.out)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Remove for compiler-specific features
set(CMAKE_CXX_EXTENSIONS OFF)

string(APPEND CMAKE_CXX_FLAGS " -Wall")
string(APPEND CMAKE_CXX_FLAGS " -Wbuiltin-macro-redefined")
string(APPEND CMAKE_CXX_FLAGS " -pedantic")
string(APPEND CMAKE_CXX_FLAGS " -Werror")

# clangd completion
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

include_directories(${CMAKE_SOURCE_DIR})
file(GLOB SOURCES "${CMAKE_SOURCE_DIR}/*.cpp")

add_executable(${PROJECT_NAME} ${SOURCES})

set(Boost_USE_STATIC_LIBS        ON) # only find static libs
set(Boost_USE_MULTITHREADED      ON)
set(Boost_USE_STATIC_RUNTIME    OFF) # do not look for boost libraries linked against static C++ std lib

find_package(Boost REQUIRED COMPONENTS filesystem)

target_link_libraries(${PROJECT_NAME}
    Boost::filesystem
)
```

_*Note that in this example, Boost is linked statically since it is required if the target machine does not have the right version of Boost pre-installed; this recommendation applies to all dependencies pre-installed in the docker image._

The Dockerfile is also very simple:

```docker
FROM ubuntu:18.04
LABEL Description="Build environment"

ENV HOME /root

SHELL ["/bin/bash", "-c"]

RUN apt-get update && apt-get -y --no-install-recommends install \
    build-essential \
    clang \
    cmake \
    gdb \
    wget

# Let us add some heavy dependency
RUN cd ${HOME} && \
    wget --no-check-certificate --quiet \
        https://boostorg.jfrog.io/artifactory/main/release/1.77.0/source/boost_1_77_0.tar.gz && \
        tar xzf ./boost_1_77_0.tar.gz && \
        cd ./boost_1_77_0 && \
        ./bootstrap.sh && \
        ./b2 install && \
        cd .. && \
        rm -rf ./boost_1_77_0
```

In order to make sure that its name does not conflict with existing docker files and represents the motive, I call it `DockerfileBuildEnv`.

Let us build our environment image:

```plain
$ docker build -t example/example_build:0.1 -f DockerfileBuildEnv .
Here is supposed to be a long output of boost build
```

_*Note that the version is not the “latest” but has a meaningful name (e.g. 0.1)._

After the image is built, we can eventually build the project. First, we need to launch a docker container based on our image and run bash inside.

```plain
$ cd project
$ docker run -it --rm --name=example \
 --mount type=bind,source=${PWD},target=/src \
 example/example_build:0.1 \
 bash
```

The only parameter that I would like to highlight here is `--mount type=bind,source=${PWD},target=/src`; it instructs docker to mount the current directory (where the source code is located) to the directory `src`. Thus, we avoid copying source files to the container and, as you will see later, store the output binaries in the host’s file system avoiding unnecessary copies. For the rest of the flags, please refer to the official docker [documentation](https://docs.docker.com/engine/reference/run/){:target="_blank"}.

Now, within the container, let us build the project:

```plain
root@3abec58c9774:/# cd src
root@3abec58c9774:/src# mkdir build && cd build
root@3abec58c9774:/src/build# cmake ..
-- The C compiler identification is GNU 7.5.0
-- The CXX compiler identification is GNU 7.5.0
-- Check for working C compiler: /usr/bin/cc
-- Check for working C compiler: /usr/bin/cc -- works
-- Detecting C compiler ABI info
-- Detecting C compiler ABI info - done
-- Detecting C compile features
-- Detecting C compile features - done
-- Check for working CXX compiler: /usr/bin/c++
-- Check for working CXX compiler: /usr/bin/c++ -- works
-- Detecting CXX compiler ABI info
-- Detecting CXX compiler ABI info - done
-- Detecting CXX compile features
-- Detecting CXX compile features - done
-- Boost  found.
-- Found Boost components:
   filesystem
-- Configuring done
-- Generating done
-- Build files have been written to: /src/build

root@3abec58c9774:/src/build# make
Scanning dependencies of target a.out
[ 50%] Building CXX object CMakeFiles/a.out.dir/main.cpp.o
[100%] Linking CXX executable a.out
[100%] Built target a.out
```

Et Voila, the project was built successfully!

The resulting binary runs successfully, both in the container and on the host, because the Boost is linked _statically_.

```plain
$ build/a.out
The size of "/home/dima/dockerized_cpp_build_example/build/a.out" is 177320
```

## Making the environment usable

At this point, you may anxiously wonder how you are expected to remember all these docker commands. A developer is not expected to know every one of these details to build a project. In order to simplify the process, I suggest wrapping docker commands with a common tool among most developers -- make.

For this purpose, I have created a GitHub [repository](https://github.com/f-squirrel/dockerized_cpp){:target="_blank"} with an easily customizable Makefile, which can be used for almost every cmake-based project without changes. The user can either download it from this repository or add it as a [git submodule](https://git-scm.com/docs/git-submodule){:target="_blank"} to get the latest version. I recommend and prefer the latter, therefore I will elaborate further.

The Makefile supports basic commands; to see the options, the user has to run `make help`:

```plain
$ make help
gen_cmake                      Generate cmake files, used internally
build                          Build source. In order to build a specific target run: make TARGET=<target name>.
test                           Run all tests
clean                          Clean build directory
login                          Login to the container. Note: if the container is already running, login into the existing one
build-docker-deps-image        Build the deps image.
```

Let us start with adding the Makefile to our sample project via git module to the directory `build_tools`:

```plain
git submodule add  https://github.com/f-squirrel/dockerized_cpp.git build_tools/
```

The next step is to create another Makefile in the root of the repository and include the Makefile that we have just checked out:

```
include build_tools/Makefile
```

The project is almost ready to be compiled, though it is recommended to change some defaults, such as declaring variables in the top-level Makefile before including  `build_tools/Makefile:`

```
PROJECT_NAME=example
DOCKER_DEPS_VERSION=0.1

include build_tools/Makefile
```

By defining the project name, we automatically set the build image name as `example/example_build`.

Make is now ready to build the image:

```plain
$ make build-docker-deps-image
docker build  -t example/example_build:latest \
 -f ./DockerfileBuildEnv .
Sending build context to Docker daemon  1.049MB
Step 1/6 : FROM ubuntu:18.04

< long output of docker build >

Build finished. Docker image name: "example/example_build:latest".
Before you push it to Docker Hub, please tag it(DOCKER_DEPS_VERSION + 1).
If you want the image to be the default, please update the following variables:
/home/dima/dockerized_cpp_build_example/Makefile: DOCKER_DEPS_VERSION
```

The Makefile tags the image as `latest` - please [tag](https://docs.docker.com/engine/reference/commandline/tag/){:target="_blank"} it with an appropriate version, which in our case is `0.1`.

Finally, let us build the project:

```plain
$ make
docker run -it --init --rm --memory-swap=-1 --ulimit core=-1 --name="example_build" --workdir=/example --mount type=bind,source=/home/dima/dockerized_cpp_build_example,target=/example  example/example_build:0.1 \
 bash -c \
 "mkdir -p /example/build && \
 cd build && \
 CC=clang CXX=clang++ \
 cmake  .."
-- The C compiler identification is Clang 6.0.0
-- The CXX compiler identification is Clang 6.0.0
-- Check for working C compiler: /usr/bin/clang
-- Check for working C compiler: /usr/bin/clang -- works
-- Detecting C compiler ABI info
-- Detecting C compiler ABI info - done
-- Detecting C compile features
-- Detecting C compile features - done
-- Check for working CXX compiler: /usr/bin/clang++
-- Check for working CXX compiler: /usr/bin/clang++ -- works
-- Detecting CXX compiler ABI info
-- Detecting CXX compiler ABI info - done
-- Detecting CXX compile features
-- Detecting CXX compile features - done
-- Boost  found.
-- Found Boost components:
   filesystem
-- Configuring done
-- Generating done
-- Build files have been written to: /example/build

CMake finished.
docker run -it --init --rm --memory-swap=-1 --ulimit core=-1 --name="example_build" --workdir=/example --mount type=bind,source=/home/dima/dockerized_cpp_build_example,target=/example  example/example_build:latest \
 bash -c \
 "cd build && \
 make -j $(nproc) "
Scanning dependencies of target a.out
[ 50%] Building CXX object CMakeFiles/a.out.dir/main.cpp.o
[100%] Linking CXX executable a.out
[100%] Built target a.out

Build finished. The binaries are in /home/dima/dockerized_cpp_build_example/build
```

Now If you take a look at the host’s build directory, you will notice that the output binary is conveniently there.

I hope this article was helpful, please feel free to reach out with any queries.

_The Github repository containing the Makefile, including a description of available variables and their default values can be found [here](https://github.com/f-squirrel/dockerized_cpp){:target="_blank"}._

_The complete example used in this post, as well as examples of overriding default values and adding new commands, can be found in the repository [Dockerized C++ Build Example](https://github.com/f-squirrel/dockerized_cpp_build_example){:target="_blank"}._

## Update

I have been honored with an invitation to speak at [Core C++ User Group](https://corecppil.github.io/Meetups/) online event about this topic.
You can check out my talk and live demo at YouTube:

<div class="embed-youtube" data-nosnippet="true">
<iframe width="1280" height="720" src="https://www.youtube.com/embed/B0DptqheF5I" title="YouTube video player" frameborder="0" allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture" allowfullscreen></iframe>
</div>

## Update #2

You can read about configuring VS Code to work with dockerized C/C++ build in my latest [post](/dockerized-cpp-build-with-vscode).

## Update #3 -- Running as a non-root user in Docker

The original version of the dockerized build ran as the root user.
In general, this is not an issue because developers could always "chmod" the result file.
However, running docker containers as root is not a good practice from the security perspective and, what is more important,
it might be an issue if one of the targets changes the source code. It is common to format code or apply clang-tidy fixes via make commands. This would result in having source files under the root user, which makes it impossible to edit them at the host.

In order to solve the issue, I have updated the sources of the dockerized build so that it runs the container as the host user by [providing](https://github.com/f-squirrel/dockerized_cpp/blob/master/Makefile#L32) the current user id and group id. From now on, this is the default behavior, if you need to change it, run make as follows:

```sh
make DOCKER_USER_ROOT=ON
```

It is important to note that the docker image does not contain the host's user, i.e. there is no home directory, name or group. It means that if your build uses the home directory, then probably this mode is not good for you.

*Special thanks to [Rina Volovich](https://www.linkedin.com/in/rina-volovich/) for editing.*

Please share your thoughts on [Twitter](https://twitter.com/dbdanilov/status/1454109694776299527?s=20), [Reddit](https://www.reddit.com/r/cpp/comments/qly3iy/dockerized_build_environments_for_cc_projects/?utm_source=share&utm_medium=web2x&context=3) or [LinkedIn](https://www.linkedin.com/posts/ddanilov_dockerized-build-environments-for-cc-projects-activity-6859875172300685312-gkl2?utm_source=share&utm_medium=member_desktop).
