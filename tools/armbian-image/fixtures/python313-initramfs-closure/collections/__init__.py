import sys

import _collections_abc

abc = _collections_abc
sys.modules["collections.abc"] = _collections_abc
