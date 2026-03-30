"""
qnap-cli: CLI for QNAP NAS management.
"""

try:
    from importlib.metadata import version
    __version__ = version("qnap-cli")
except ImportError:
    from importlib_metadata import version
    __version__ = version("qnap-cli")
