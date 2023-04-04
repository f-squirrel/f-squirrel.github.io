---
title: Python garbage collector and the other magic
published: true
permalink: "/python-garbage-collector-and-other-magic/"
cover-img: /img/python_garbage_collection_header.jpeg
tags: [python, debugging, garbage collection]
readtime: true
comments: false
---

Once upon a time, in the company where I work there was a bug in production that caused to the `Too many files open` error.

I was able to locate the class responsible for the issue, it was a class working with os.pipefile descriptors, let’s call it OsPipeHolder. You may find the simplified version of the code below:

```python
#Listing #1

#!/usr/bin/python
import os


class OsPipeHolder(object):
    def __init__( self ):
        read, write = os.pipe()
        self._read = os.fdopen( read, "r" )
        self._write = os.fdopen( write, "w" )
        self.isClosed = self.is_closed

    def is_closed(self):
        return self._read and self._write

    def close(self):
        self._read.close()
        self._write.close()
        self._read = None
        self._write = None

    def __del__(self):
        print "You've deleted me!!!"
        if not self.is_closed():
            self.close()


if __name__ == "__main__":
    pipe = OsPipeHolder()
    del pipe
```

The problem was that the application made a retry in the event of
failure and created a new instance of `OsPipeHolder` class.

As you may see from the code above there is `__del__` method that should be called by the garbage collector.

But… for no reason, it is never called and the file descriptors are left open:
<pre>
$ ./OsPipeHolder.py
$
</pre>

As the reader may know, Python’s garbage collector destroys objects not referenced from the stack(1 or fewer references).

Despite we create only one instance of the object in the line #30 and do not copy it elsewhere, I propose to verify the number of references with `sys.getrefcount`:

```python
#Listing #2

import os
import sys

class OsPipeHolder(object):
...

if __name__ == "__main__":
    pipe = OsPipeHolder()
    #we need -1 since passing an object to getrefcount
    #creates an additional reference
    print "Refcount:", (sys.getrefcount(pipe) -1)               
    del pipe
```

Run results #2:
<pre>
$ ./OsPipeHolder_refcount.py
Refcount: 2
</pre>
As you see, every time we create an instance of `OsPipeHolder` Python creates **two** references!
So, maybe there is an internal reference inside the object itself.

In order to check it I have decided to print the information on all the attributes of `OsPipeHolder`:

```python
#Listing #3

#!/usr/bin/python
import os
import sys

class OsPipeHolder(object):
...

if __name__ == "__main__":
    pipe = OsPipeHolder()
    #we need -1 since passing an object to getrefcount
    #creates an additional reference
    print "Refcount:", (sys.getrefcount(pipe) -1)          
    for i, attribute in enumerate(dir(pipe)):
            msg = "%d. Attribute name: %s\tinfo: %s" % (i, attribute, (getattr(pipe, attribute)))
            print msg

    del(pipe)
```

Run results #3:
<pre>
$ ./OsPipeHolder.py
Refcount: 2
0. Attribute name: __class__ info: <class '__main__.OsPipeHolder'>
1. Attribute name: __del__ info: <bound method OsPipeHolder.__del__ of <__main__.OsPipeHolder object at 0x7fe84a3bc110>>
...
21. Attribute name: close info: <bound method OsPipeHolder.close of <__main__.OsPipeHolder object at 0x7fe84a3bc110>>
22. Attribute name: <span style="background-color: #FFFF00">isClosed    info: <bound method OsPipeHolder.is_close of <__main__.OsPipeHolder object at 0x7fe84a3bc110>></span>
23. Attribute name: is_close    info: <bound method OsPipeHolder.is_close of <__main__.OsPipeHolder object at 0x7fe84a3bc110>>
</pre>

At first glance everything looks good, but in the line #22 we see that `isClosed` is a reference to the method `is_close`.
This is the inner reference cycle we were looking for!

Let’s comment it out:

```python
#Listing #4

#!/usr/bin/python
import os
import sys

class OsPipeHolder(object):
    def __init__( self ):
        read, write = os.pipe()
        self._read = os.fdopen( read, "r" )
        self._write = os.fdopen( write, "w" )
        #self.isClosed = self.is_closed
...
```

Run results #4:

```sh
$ ./OsPipeHolder.py
Refcount: 1
You've deleted me!!!
```

Yey, finally our method `__del__` was called!

But we still have two issues:

1. We cannot remove `isClosed` since it was added for backward compatibility purposes
1. Python is supposed to handle reference cycles easily!

Let’s start with the first.<br>
I found the solution in Python sources.
In order to create an alias to a function you simply declare `isClosed` as a "class level attribute"(line #16).

```python
#Listing #5

#!/usr/bin/python
import os
import sys

class OsPipeHolder(object):
    def __init__( self ):
        read, write = os.pipe()
        self._read = os.fdopen(read, "r")
        self._write = os.fdopen(write, "w")

    def is_closed(self):
        return self._read and self._write

    isClosed = is_closed

    def close(self):
        self._read.close()
        self._write.close()
        self._read = None
        self._write = None

    def __del__(self):
        print "You've deleted me!!!"
        if not self.is_closed():
            self.close()

if __name__ == "__main__":
    pipe = OsPipeHolder()
    print "Refcount:", (sys.getrefcount(pipe) -1)
    del(pipe)
```

Run results #5:
<pre>
$ ./OsPipeHolder.py
Refcount: 1
You've deleted me!!!
$ ./OsPipeHolder.py
Refcount: 1
You've deleted me!!!
</pre>

As you see, we have only one reference to the instance and the garbage collector calls our `__del__` method!
But why isn’t it called in the original code?
Let's take a look at the memory layout of our object:

![Memory layout](/img/mem_layout.jpeg)

As you see we have one reference(Ref #1) to the object from the stack and another inner reference (Ref #2).
After the Ref #1 is deleted and we have no other references from the stack the garbage collector is supposed to call the `__del__` method despite the inner reference.
Why does it not happen?

Python documentation is your best friend and has an answer for everything!

> A list of objects which the collector found to be unreachable but could not be freed (uncollectable objects).
> By default, this list contains only objects with `__del__()` methods.
> [1](https://docs.python.org/2/library/gc.html#id2) Objects that have `__del__()` methods and are part of a reference cycle
> cause the entire reference cycle to be uncollectable,
> including objects not necessarily in the cycle but reachable only from it.
>
> *Python docs: [https://docs.python.org/2/library/gc.html#gc.garbage](https://docs.python.org/2/library/gc.html#gc.garbage){:target="_blank"}*

As you see, the `__del__` method itself was the root cause!

## Summary ##

* Don’t create aliases with `self` since it leads to a redundant
reference(only `alias = method_name` and not `self.alias = self.method_name`)
* Remember the Zen of Python saying: “Explicit is better than implicit.” (call `close` method explicitly or use the `with` statement)
* Python is not C++ so do not implement [RAII](https://en.wikipedia.org/wiki/Resource_acquisition_is_initialization){:target="_blank"} there
* Think in the Pythonic way and read the docs!:)

P.S. Originally I posted it at [Medium](https://medium.com/@dimadanilov_71824/python-garbage-collector-and-other-magic-c563f9e959f9){:target="_blank"}
