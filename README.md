# aliyundrive-webdav

[![GitHub Actions](https://github.com/messense/aliyundrive-webdav/workflows/CI/badge.svg)](https://github.com/messense/aliyundrive-webdav/actions?query=workflow%3ACI)
[![PyPI](https://img.shields.io/pypi/v/aliyundrive-webdav.svg)](https://pypi.org/project/aliyundrive-webdav)

阿里云盘 WebDAV 服务

## 安装

可以从 [GitHub Releases](https://github.com/messense/aliyundrive-webdav/releases) 页面下载预先构建的二进制包，
也可以使用 pip 从 PyPI 下载:

```bash
pip install aliyundrive-webdav
```

## 用法

```bash
aliyundrive-webdav --help
aliyundrive-webdav 0.1.10

USAGE:
    aliyundrive-webdav [FLAGS] [OPTIONS] --refresh-token <refresh-token>

FLAGS:
    -I, --auto-index    Automatically generate index.html
    -h, --help          Prints help information
    -V, --version       Prints version information

OPTIONS:
    -W, --auth-password <auth-password>          WebDAV authentication password [env: WEBDAV_AUTH_PASSWORD=]
    -U, --auth-user <auth-user>                  WebDAV authentication username [env: WEBDAV_AUTH_USER=]
        --host <host>                            Listen host [default: 127.0.0.1]
    -p, --port <port>                            Listen port [default: 8080]
    -S, --read-buffer-size <read-buffer-size>
            Read/download buffer size in bytes, defaults to 10MB [default: 10485760]

    -r, --refresh-token <refresh-token>          Aliyun drive refresh token [env: REFRESH_TOKEN=]
```

### 获取 refresh_token

登录[阿里云盘](https://www.aliyundrive.com/drive/)后，可以在开发者工具 ->
Application -> Local Storage 中的 `token` 字段中找到。

## License

This work is released under the MIT license. A copy of the license is provided in the [LICENSE](../LICENSE) file.
