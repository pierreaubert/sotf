#!/usr/bin/env python3
from http.server import HTTPServer, SimpleHTTPRequestHandler
import sys

class CORSRequestHandler(SimpleHTTPRequestHandler):
    def end_headers(self):
        # Add COOP/COEP headers for SharedArrayBuffer (parallel WASM)
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        self.send_header('Access-Control-Allow-Origin', '*')
        super().end_headers()

if __name__ == '__main__':
    port = 8080
    server = HTTPServer(('localhost', port), CORSRequestHandler)
    print(f"Server running at http://localhost:{port}")
    print("COOP/COEP headers enabled for parallel WASM")
    server.serve_forever()
