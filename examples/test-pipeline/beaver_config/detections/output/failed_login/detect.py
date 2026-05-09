import re, json, functools, ipaddress
from fnmatch import fnmatch


def detect(record):
    return record.get("event") == "login" and record.get("status") == "failed"
