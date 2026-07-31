import collections.abc
import fcntl
import math
import _posixsubprocess
import select
import _struct
import zlib

assert collections.abc.__file__.endswith("_collections_abc.py")
