# atrium-oauth-axum

A demo web application for OAuth built using [`atrium`](https://github.com/atrium-rs/atrium) and [`axum`](https://github.com/tokio-rs/axum). This project is a Rust-based port of the Python implementation found at [Bluesky Social's Cookbook](https://github.com/bluesky-social/cookbook/tree/main/python-oauth-web-app).

Currently, the application is hosted on [`render.com`](https://atrium-oauth-axum.onrender.com/). It is designed to be environment-agnostic and can run anywhere, provided it is connected to a Redis instance.

## Requirements

- Rust `1.83` or later

## Development

To set up the development environment, follow these steps:

```bash
cp .env.example .env
# Edit the .env file with your configuration
dotenvx run -- cargo run
```
