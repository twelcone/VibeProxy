// Emit `usage/index.html` rather than `usage.html`, so the file is served at pathname `/usage/`.
// A file at `/usage.html` loads fine but the client router then looks for a route matching
// `/usage.html`, finds none, and renders its own 404 — which is what shipped.
export const prerender = true;
export const trailingSlash = "always";
