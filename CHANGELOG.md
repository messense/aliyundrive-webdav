# Changelog

All notable changes to this project will be documented in this file.

## 1.8.0

* 增加配置上传文件缓冲区大小参数 `--upload-buffer-size`
* 增加配置跳过上传相同大小同名文件参数 `--skip-upload-same-size`, 注意启用该选项虽然能加速上传但可能会导致修改过的同样大小的文件不会被上传

## 1.7.4

* 删除文件时忽略 404 和 400 状态码
* 修复梅林 arm384/arm386 使用 usb2jffs 插件后安装报错 `bad number` 问题
* 上传文件出错时日志中增加更详细的错误信息

## 1.7.3

* 调用云盘接口增加自动重试机制

## 1.7.2

* 增加 socks5 代理支持

## 1.7.1

* OpenWrt Luci 配置界面增加扫码登录获取 refresh token 功能

## 1.7.0

* 梅林 384/386 固件禁用程序自动更新
* 默认使用 App refresh token 刷新接口
* 增加 `aliyundrive-webdav qr` 子命令

## 1.6.2

* 非 tty 终端模式下不尝试扫码登录

## 1.6.1

* 降低自动更新失败日志级别为警告

## 1.6.0

* 增加自动更新功能

## 1.5.1

* 修复 Web 版 refresh token 刷新失败问题

## 1.5.0

* 增加移动端 App refresh token 支持,扫码登录使用 App refresh token.

## 1.4.0

* 命令行增加阿里云盘扫码登录功能

## 1.3.3

* 增加 `--strip-prefix` 参数

## 1.3.2

* 不使用阿里云盘文件列表接口返回的可能有问题的图片下载地址

## 1.3.1

* 降低获取文件下载地址接口调用次数

## 1.3.0

* 支持下载 `.livp` 格式文件

## 1.2.7

* 修复下载部分文件类型如 `.livp` 500 报错问题，由于阿里云盘接口没有返回 `.livp` 文件格式下载地址，暂时无法下载该格式文件

## 1.2.6

* 指定 `--workdir` 参数时 `--refresh-token` 参数可选

## 1.2.5

* 修复 Windows 版本访问文件 404 问题

## 1.2.4

* 修正 OpenWrt package autorelease 版本号

## 1.2.3

* 增加 Windows arm64 架构支持

## 1.2.2

* TLS/HTTPS 支持 RSA 私钥格式

## 1.2.1

* 支持 OpenWrt 19.07

## 1.2.0

* 增加 TLS/HTTPS 支持（暂不支持 MIPS 架构）
* 增加 HTTP 2.0 支持
* 修复 Docker 容器设置 `HOST` 环境变量不生效的问题
* 增加构建发布 deb 和 rpm 包

## 1.1.1

* 修复潜在的内存泄漏问题

## 1.1.0

* 增加只读模式，防止误操作删除文件

## 1.0.0

* 调整连接池 idle 检测时间，避免下载文件时出现 `connection closed before message
  completed` 报错
* 功能相对稳定，发布 1.0 版本。

## 0.5.5

* 降级 OpenSSL 修复 MIPS 架构二进制文件无法正常运行的问题

## 0.5.4

* 刷新 refresh token 增加 429 状态码重试

## 0.5.3

* 完善请求重试，处理请求 408、429 报错

## 0.5.2

* 增加 `arm_cortex-a5_vfpv4` 架构 OpenWrt 包（玩客云适用）

## 0.5.1

* 修复 OpenWrt Luci 界面语言翻译问题

## 0.5.0

* 增加实验性[阿里云相册与网盘服务（PDS）](https://www.aliyun.com/product/storage/pds)支持，阿里云网站开通 PDS 服务后可通过传入 `domain_id` 和对应用户的 `refresh_token`（可通过访问 BasicUI 获取） 使用。

## 0.4.8

* 支持通过环境变量 `HOST` 和 `PORT` 配置监听地址和端口

## 0.4.7

* 发布 musllinux wheel 二进制包至 PyPI

## 0.4.6

* 自动尝试刷新过期的上传地址
* GitHub Release 产物文件名增加版本号

## 0.4.5

* 兼容 macOS Finder chunked encoding 上传 `X-Expected-Entity-Length` HTTP header

## 0.4.4

* 新增目录缓存过期时间参数配置
