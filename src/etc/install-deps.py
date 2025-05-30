#!/usr/bin/env python

import contextlib
import download
import os
import shutil
import sys
import tarfile

if os.environ.get('BITS') == '32':
    host_bits = 'i686'
    extra_bits = 'x86_64'
else:
    host_bits = 'x86_64'
    extra_bits = 'i686'



# Figure out our target triple
if sys.platform == 'linux' or sys.platform == 'linux2':
    host = host_bits + '-unknown-linux-gnu'
    targets = [
        'i686-unknown-linux-gnu',
        'x86_64-unknown-linux-gnu',
        'x86_64-unknown-linux-musl',
        'arm-unknown-linux-gnueabi',
        'arm-unknown-linux-gnueabihf',
        'armv7-unknown-linux-gnueabihf',
        'x86_64-unknown-freebsd',
        'x86_64-unknown-netbsd',
    ]
elif sys.platform == 'darwin':
    host = host_bits + '-apple-darwin'
    targets = ['i686-apple-darwin', 'x86_64-apple-darwin']
elif sys.platform == 'win32':
    if os.environ.get('MSVC') == '1':
        host = host_bits + '-pc-windows-msvc'
        targets = [
            'i686-pc-windows-msvc',
            'x86_64-pc-windows-msvc',
        ]
    else:
        host = host_bits + '-pc-windows-gnu'
        targets = [host]
else:
    exit_msg = "There is no official Cargo snapshot for {} platform, sorry."
    sys.exit(exit_msg.format(sys.platform))

rust_date = open('src/rustversion.txt').read().strip()
url = 'https://static.rust-lang.org/dist/' + rust_date


def install_via_tarballs():
    if os.path.isdir("rustc-install"):
        shutil.rmtree("rustc-install")

    # Download the compiler
    host_fname = 'rustc-nightly-' + host + '.tar.gz'
    download.get(url + '/' + host_fname, host_fname)
    download.unpack(host_fname, "rustc-install", quiet=True, strip=2)
    os.remove(host_fname)

    # Download all target libraries needed
    for target in targets:
        fetch_std(target)

    if os.path.isdir("rustc"):
        shutil.rmtree("rustc")
    os.rename("rustc-install", "rustc")

def fetch_std(target):
    fname = 'rust-std-nightly-' + target + '.tar.gz'
    print("adding target libs for " + target)
    download.get(url + '/' + fname, fname)
    download.unpack(fname, "rustc-install", quiet=True, strip=2)
    os.remove(fname)

install_via_tarballs()
