# Changelog

All notable changes to this project will be documented in this file.

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
