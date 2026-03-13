from http.server import HTTPServer, SimpleHTTPRequestHandler
import os

PORT = 5500
# Ensure this script points to your 'public' folder
web_dir = os.path.join(os.path.dirname(__file__), "public")
os.chdir(web_dir)

class AASAHandler(SimpleHTTPRequestHandler):
    def end_headers(self):
        # Check if the requested path is the AASA file
        if self.path.endswith('apple-app-site-association'):
            self.send_header("Content-Type", "application/json")
        super().end_headers()

httpd = HTTPServer(("", PORT), AASAHandler)
print(f"Serving {web_dir} on port {PORT}")
httpd.serve_forever()