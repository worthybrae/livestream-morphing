#!/usr/bin/env python3
"""Serve a local collection with single-byte-range video seeking."""
import argparse
import functools
import http.server
import os
import re

class Handler(http.server.SimpleHTTPRequestHandler):
    def send_head(self):
        self.remaining = None
        path = self.translate_path(self.path)
        value = self.headers.get('Range')
        if not value or not os.path.isfile(path):
            return super().send_head()
        match = re.fullmatch(r'bytes=(\d*)-(\d*)', value.strip())
        size = os.path.getsize(path)
        if match and any(match.groups()):
            first, last = match.groups()
            start = int(first) if first else max(0, size - int(last))
            end = min(int(last), size - 1) if first and last else size - 1
            if 0 <= start <= end < size:
                stream = open(path, 'rb')
                stream.seek(start)
                self.remaining = end - start + 1
                self.send_response(206)
                self.send_header('Content-Type', self.guess_type(path))
                self.send_header('Content-Length', str(self.remaining))
                self.send_header('Content-Range', f'bytes {start}-{end}/{size}')
                self.send_header('Accept-Ranges', 'bytes')
                self.end_headers()
                return stream
        self.send_response(416)
        self.send_header('Content-Range', f'bytes */{size}')
        self.send_header('Content-Length', '0')
        self.end_headers()
        return None

    def end_headers(self):
        if self.remaining is None:
            self.send_header('Accept-Ranges', 'bytes')
        super().end_headers()

    def copyfile(self, source, output):
        if self.remaining is None:
            return super().copyfile(source, output)
        while self.remaining:
            data = source.read(min(65536, self.remaining))
            if not data:
                break
            output.write(data)
            self.remaining -= len(data)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory')
    parser.add_argument('--port', type=int, default=3089)
    args = parser.parse_args()
    handler = functools.partial(Handler, directory=os.path.abspath(args.directory))
    with http.server.ThreadingHTTPServer(('127.0.0.1', args.port), handler) as server:
        print(f'Collection: http://127.0.0.1:{args.port}/', flush=True)
        server.serve_forever()
