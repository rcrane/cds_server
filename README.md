# CDS Lab Evaluation Server

This is the CDS LAB Evaluation Server.
A tiny web service that invokes programs on the command line and measures their execution time.

## Configuration

The server reads its configuration from a json-encoded file.
The configuration is merely a mapping of program names to program paths.
For example, the following configuration will instruct the server to invoke `/bin/cat` whenever `echo` is requested via the API's `run` endpoint.

```json
[
	["echo", "/bin/cat"]
]
```

## Command-line arguments

The server knows two command-line arguments:

```bash
$ ./target/debug/cds_server --help
cds_server 0.1.0
Franz Gregor <franz.gregor@tu-dresden.de>
A simple REST-API server forwards requests to binaries and measures their execution time

USAGE:
    cds_server [OPTIONS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --config <config>    Server configuration file (default: ~/.config/cds_server.json)
    -p, --port <port>        TCP/IP port the server will listen on (default: 8080)
```

## API

The server exposes a single API-endpoint: `/run`.

### Default behavior

If the HTTP request can not be matched to another endpoint the server will return a human readable message given a short introduction and listing the program names the server is able to invoke/run.

### Run a program: `/run/:program`

The run endpoint instructs the server to start a program and measure its execution time.
The request's body contains the program input that will be written to the program's stdin stream.
The response contains the program's exit status/return code, the collected output of its `stdout` and `stderr` streams, and the measured runtime in microseconds or an error message.
Fields `stdin`, `stdout`, and `stderr` are base64 encoded to prevent issues with the protocols json-encoding.

* Method: POST
* Example Request Body:

  ```json
    {
        "stdin": "aGVsbG8gd29ybGQK"
    }
  ```

* Example Response Body:

  ```json
    {
        "stdout": "aGVsbG8gd29ybGQK",
        "stderr": "",
        "exit_status": 0,
        "duration": 412,
        "error": null
    }
  ```