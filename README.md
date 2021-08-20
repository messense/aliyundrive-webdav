# aliyundrive-webdav

[![GitHub Actions](https://github.com/messense/aliyundrive-webdav/workflows/CI/badge.svg)](https://github.com/messense/aliyundrive-webdav/actions?query=workflow%3ACI)
[![PyPI](https://img.shields.io/pypi/v/aliyundrive-webdav.svg)](https://pypi.org/project/aliyundrive-webdav)

阿里云盘 WebDav 服务

## Installation

You can download prebuilt binaries from [GitHub Releases](https://github.com/messense/aliyundrive-webdav/releases).
Or install it from PyPI:

```bash
pip install aliyundrive-webdav
```

## Usage

```bash
aliyundrive-webdav --help
aliyundrive-webdav 0.1.0

USAGE:
    aliyundrive-webdav [FLAGS] [OPTIONS] --refresh-token <refresh-token>

FLAGS:
    -I, --auto-index    Automatically generate index.html
    -h, --help          Prints help information
    -V, --version       Prints version information

OPTIONS:
    -W, --auth-password <auth-password>    WebDav authentication password
    -U, --auth-user <auth-user>            WebDav authentication username
        --host <host>                      Listen host [default: 127.0.0.1]
    -p, --port <port>                      Listen port [default: 8080]
    -r, --refresh-token <refresh-token>    Aliyun drive refresh token [env: REFRESH_TOKEN=]
```

## License

This work is released under the MIT license. A copy of the license is provided in the [LICENSE](../LICENSE) file.
