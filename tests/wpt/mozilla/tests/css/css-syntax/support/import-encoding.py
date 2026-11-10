# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.

def main(request, response):
    mode = request.GET.first(b"mode", b"")
    response.headers.set(b"Content-Type", b"text/css")
    if mode == b"parent-direct":
        response.headers.set(b"Content-Type", b"text/css; charset=windows-1251")
        return b'@import url("import-encoding.py?mode=child-direct");'
    if mode == b"parent-nested":
        response.headers.set(b"Content-Type", b"text/css; charset=windows-1251")
        return b'@import url("import-encoding.py?mode=middle");'
    if mode == b"parent-declared":
        return b'@charset "windows-1251";@import url("import-encoding.py?mode=child-declared");'
    if mode == b"parent-bom":
        css = '@import url("import-encoding.py?mode=child-bom");'
        return b"\xff\xfe" + css.encode("utf-16le")
    if mode == b"middle":
        return b'@import url("import-encoding.py?mode=child-nested");'
    if mode == b"child-direct":
        return "#дочь { color: rgb(0, 128, 0) }".encode("windows-1251")
    if mode == b"child-nested":
        return "#внук { color: rgb(0, 128, 0) }".encode("windows-1251")
    if mode == b"child-declared":
        return "#кодировка { color: rgb(0, 128, 0) }".encode("windows-1251")
    if mode == b"child-bom":
        return "#байт { color: rgb(0, 128, 0) }".encode("utf-16le")
    return b""
