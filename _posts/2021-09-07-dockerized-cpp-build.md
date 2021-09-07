---
title: Dockerized C++ build environment
published: true
share-description: ""
tags: [cpp, docker, build]
share-img: /img/docker-logo-696x364.png
readtime: true
permalink: "/dockerized-cpp-build"
share-description: "How to create a convenient C++ build environment in docker"
---

Nowadays, docker is a de facto standard way of deploying applications. Also, it
can be very useful for building C++ applications: since C++ does not have a
built-in dependency management mechanism, it is usually a mix of different
techniques: installing dependencies from Linux distro’s repositories (e.g.
apt-get), adding 3rd parties as submodules, and building them within
the source tree, or using some half-baked solutions like Conan. Unfortunately,
all of them have certain disadvantages: 

* The dependencies installed on the dev machine make the dev environment dirty and almost never the same or CI/CD and production.
* Adding 3rd parties as submodules slows down the build so significantly that developers get afraid to clean a build directory or swtiching major branches.
* Solutions like Conan often lack support of certain libraries or their
versions and adding them requires writing code in Python which from my point of
view is a bit too much.


Docker helps to solve these problems.
The idea is to build a project in a container and put the binaries on the host filesystem.
The major advantage of this approach is the ability to have a single, controllable and reproducible build environment:
* All the tools, dependencies and configurations are part of the docker image,
    which means that everyone including the CI/CD uses the same environment.
* All versions of build environment (image versions) stored in an artifactory.
* Every party no matter if it is developer's workstation or CI/CD use the same build environment.


First of all, let us create a simple C++ application printing the size of
itself with boost::filesystem as a dependency.

```cpp
#include <boost/filesystem/operations.hpp>
#include <iostream>

int main(int argc, char *argv[]) {
    std::cout << "The size of " << boost::filesystem::absolute(argv[0])
              << " is " << boost::filesystem::file_size(argv[0]) << '\n';
    return 0;
}
```
The Cmake file is quite standard so I am not going to elaborate on it, just leave a [link] to GitHub.

Usually, most projects have some sort of installation script to install/build all needed dependencies, let us call it install_deps.sh. In my case, it will look in the following way:

```sh
#!/bin/bash

set -ex

APT_GET_FLAGS="-y --no-install-recommends"
# Ideally certificates have to be updated
WGET_FLAGS="--no-check-certificate --quiet"

# Install tools
apt-get update && apt-get ${APT_GET_FLAGS} install \
    build-essential \
    clang \
    cmake \
    gdb \
    wget

# Let us add some heavy dependency
cd ${HOME}
wget ${WGET_FLAGS} \
    https://boostorg.jfrog.io/artifactory/main/release/1.77.0/source/boost_1_77_0.tar.gz && \
    tar xzf ./boost_1_77_0.tar.gz && \
    cd ./boost_1_77_0 && \
    ./bootstrap.sh && \
    ./b2 install && \
    cd .. && \
    rm -rf ./boost_1_77_0
```

Now, we are ready to create a build image. In order to do this, we have to create a docker file that runs the install_deps.sh script:

```docker
FROM ubuntu:18.04
LABEL Description="Build environment"

ENV HOME /root
COPY ./install_deps.sh /install_deps.sh

SHELL ["/bin/bash", "-c"]

RUN echo $'path-exclude /usr/share/doc/* \n\
path-exclude /usr/share/doc/*/copyright \n\
path-exclude /usr/share/man/* \n\
path-exclude /usr/share/groff/* \n\
path-exclude /usr/share/info/* \n\
path-exclude /usr/share/lintian/* \n\
path-exclude /usr/share/linda/*' > /etc/dpkg/dpkg.cfg.d/01_nodoc && \
        $SHELL "/install_deps.sh" && \
        apt-get clean && \
        apt-get autoclean && \
        rm -rf /install_deps

ENV LC_ALL=C.UTF-8
```

In this file, docker copies the installation script to the docker image and
runs it, afterwards and cleans all unnecessary dependencies, I recommend adding
here all the tools like git, wget, etc that are needed only to create the
image. Note that all the commands in the installation script can be added directly to the Dockerfile.

Those who are familiar with docker are probably expecting to build the image
now but I’d like to get there a bit later. Despite docker is super popular
these days, most developers do not remember all the flags and tricks of this
amazing application and, to be honest, they don’t have to. However, there is
another amazing tool that developers know and like to use: make. The idea is to
wrap all the docker commands like build, run, etc in a Makefile and set them as
targets. This is the list of the most basic examples:

```make
BASIC_RUN_PARAMS?=-it --init
              --name=build \
              --workdir=/project \
              --mount type=bind,source=${CURDIR},target=/project \
              build-image:latest

.PHONY: gen_cmake
gen_cmake: ## Generate cmake files, used internally
	docker run ${BASIC_RUN_PARAMS} \
		bsah -c \
		"mkdir -p /project/build && \
		cd build && \
		cmake .."
	@echo
	@echo "CMake finished."

.PHONY: build
build: gen_cmake ## Build source. In order to build a specific target run: make TARGET=<target name>.
	docker run ${BASIC_RUN_PARAMS} \
		bash -c \
		"cd build && \
		make -j $$(nproc)"
	@echo
	@echo "Build finished. The binaries are in ${CURDIR}/build"

.PHONY: build-docker-deps-image
build-docker-deps-image: ## Build the deps image. Note: without caching
	docker build -t build:latest \
		-f ./Dockerfiledeps .
	@echo
	@echo "Build finished. Docker image name: build:latest."
```

where
* `BASIC_RUN_PARAMS` - the list of common docker commands: launch in the interactive mode, mount the source directory in Docker container.
* `gen_cmake` is an internal target that generates cmake files
* `build` actually launces make, note that it uses the maximum number of CPUs available for the container.
* `build-docker-deps-image` - builds the image with the environment.

The usage of these commands is very simple:
* Build the image once by running `make build-docker-deps-image`
* Build the project `make build`

The output will be stored at the host’s build directory.

I have created a repository with more enhanced Makefile at Github, it supports the help command:
```plain
$ make help
gen_cmake                      Generate cmake files, used internally
build                          Build source. In order to build a specific target run: make TARGET=<target name>.
test                           Run all tests
clean                          Clean build directory
login                          Login to the container. Note: if the container is already running, login into existing one
build-docker-deps-image        Build the deps image. Note: without caching
```

