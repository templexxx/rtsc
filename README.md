RTSC
===

Get unix time (nanoseconds) in blazing low latency with high precision. About 5xx faster than SystemTime::now().

## Performance

| OS             |CPU           | benchmark         |    rtsc::unix_nano()   |  rtsc::unix_nano_std()    |
|----------------|--------------------|-------------------|----------------|---------------|-------------|
| macOS Monterey |Intel Core i7-7700HQ| bench & bench_std |    7 ns/iter (+/- 1)        |  32 ns/iter (+/- 2) |

## Usage

1. `rtsc::init()` for init env and functions. It'll find out tsc clock is reliable or not. Not thread safe.
2. `rtsc::unix_nano()` for get timestamp.
3. `rtsc::calibrate()` for calibrate the clock. Invoke it every 5 min is a good idea in background.

## Details

You could find a [Go version](https://github.com/templexxx/tsc) written me which explains how it works.
