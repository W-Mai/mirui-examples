#!/usr/bin/env python3
"""Resolve a NuttX release or upstream HEAD to an OS/apps commit pair."""

import argparse
import re
import subprocess


RELEASE_INDEX = "https://downloads.apache.org/nuttx/"
VERSION = r"[0-9]+\.[0-9]+\.[0-9]+"


def index(path):
    return subprocess.check_output(
        ["curl", "--fail", "--silent", "--show-error", "--location",
         "--retry", "2", "--max-time", "25", RELEASE_INDEX + path],
        text=True,
    )


def latest_release():
    versions = re.findall(rf'href="({VERSION})/"', index(""))
    if not versions:
        raise RuntimeError("Apache's NuttX release index contained no release directories")
    version = max(versions, key=lambda value: tuple(map(int, value.split("."))))
    files = set(re.findall(r'href="([^"]+)"', index(f"{version}/")))
    expected = {f"apache-nuttx-{version}.tar.gz", f"apache-nuttx-apps-{version}.tar.gz"}
    if not expected <= files:
        raise RuntimeError(f"Apache NuttX {version} is missing a release archive")
    return version


def resolve(repository, ref):
    peeled = ref + "^{}"
    output = subprocess.check_output(
        ["git", "ls-remote", "--exit-code", f"https://github.com/apache/{repository}.git", ref, peeled],
        text=True,
        timeout=30,
    )
    refs = {name: sha for sha, name in (line.split() for line in output.splitlines())}
    sha = refs.get(peeled, refs.get(ref, ""))
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise RuntimeError(f"Could not resolve {repository} {ref} to a commit SHA")
    return sha


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream", help="latest, head, or a release number such as 13.1.0")
    args = parser.parse_args()
    version = latest_release() if args.upstream == "latest" else args.upstream
    if version == "head":
        ref = "HEAD"
    elif re.fullmatch(VERSION, version):
        ref = f"refs/tags/nuttx-{version}"
    else:
        parser.error("expected latest, head, or a release number such as 13.1.0")

    nuttx = resolve("nuttx", ref)
    apps = resolve("nuttx-apps", ref)
    print(f"version={version}\nnuttx_ref={nuttx}\napps_ref={apps}")


if __name__ == "__main__":
    main()
