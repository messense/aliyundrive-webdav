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

### OpenWrt 路由器

[GitHub Releases](https://github.com/messense/aliyundrive-webdav/releases) 中有预编译的 ipk 文件，
目前提供了 aarch64 和 arm 两个版本，可以下载后使用 opkg 安装，比如

```bash
wget https://github.com/messense/aliyundrive-webdav/releases/download/v0.1.12/aliyundrive-webdav_0.1.12-0_aarch64_generic.ipk
wget https://github.com/messense/aliyundrive-webdav/releases/download/v0.1.12/luci-app-aliyundrive-webdav_0.1.12-0_all.ipk
opkg install aliyundrive-webdav_0.1.12-0_aarch64_generic.ipk
opkg install luci-app-aliyundrive-webdav_0.1.12-0_all.ipk
```

### Koolshare 梅林固件

[GitHub Releases](https://github.com/messense/aliyundrive-webdav/releases) 中有预编译包 `aliyundrivewebdav-merlin-arm*.tar.gz`，
目前只提供了旧的 arm380 固件的版本，可在下载后更改文件名为 `aliyundrivewebdav.tar.gz` 后在软件中心离线安装。

![梅林配置界面](./doc/merlin.png)

## 命令行用法

```bash
aliyundrive-webdav --help
aliyundrive-webdav 0.1.12

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
