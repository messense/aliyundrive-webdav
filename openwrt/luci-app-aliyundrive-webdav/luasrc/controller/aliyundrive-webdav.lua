module("luci.controller.aliyundrive-webdav", package.seeall)

function index()
	if not nixio.fs.access("/etc/config/aliyundrive-webdav") then
		return
	end

	local page
	page = entry({ "admin", "services", "aliyundrive-webdav" }, alias("admin", "services", "aliyundrive-webdav", "client"),
		_("AliyunDrive WebDAV"), 10) -- 首页
	page.dependent = true
	page.acl_depends = { "luci-app-aliyundrive-webdav" }

	entry({ "admin", "services", "aliyundrive-webdav", "client" }, cbi("aliyundrive-webdav/client"), _("Settings"), 10).leaf = true -- 客户端配置
	entry({ "admin", "services", "aliyundrive-webdav", "log" }, form("aliyundrive-webdav/log"), _("Log"), 30).leaf = true -- 日志页面

	entry({ "admin", "services", "aliyundrive-webdav", "status" }, call("action_status")).leaf = true -- 运行状态
	entry({ "admin", "services", "aliyundrive-webdav", "logtail" }, call("action_logtail")).leaf = true -- 日志采集
	entry({ "admin", "services", "aliyundrive-webdav", "qrcode" }, call("action_generate_qrcode")).leaf = true -- 生成扫码登录二维码地址和参数
	entry({ "admin", "services", "aliyundrive-webdav", "query" }, call("action_query_qrcode")).leaf = true -- 查询扫码登录结果
end

function action_status()
	local e = {}
	e.running = luci.sys.call("pidof aliyundrive-webdav >/dev/null") == 0
	e.application = luci.sys.exec("aliyundrive-webdav --version")
	luci.http.prepare_content("application/json")
	luci.http.write_json(e)
end

function action_logtail()
	local fs = require "nixio.fs"
	local log_path = "/var/log/aliyundrive-webdav.log"
	local e = {}
	e.running = luci.sys.call("pidof aliyundrive-webdav >/dev/null") == 0
	if fs.access(log_path) then
		e.log = luci.sys.exec("tail -n 100 %s | sed 's/\\x1b\\[[0-9;]*m//g'" % log_path)
	else
		e.log = ""
	end
	luci.http.prepare_content("application/json")
	luci.http.write_json(e)
end

function action_generate_qrcode()
	local output = luci.sys.exec("aliyundrive-webdav qr generate")
	luci.http.prepare_content("application/json")
	luci.http.write(output)
end

function action_query_qrcode()
	local data = luci.http.formvalue()
	local t = data.t
	local ck = data.ck
	local output = {}
	output.refresh_token = luci.sys.exec("aliyundrive-webdav qr query --t " .. t .. " --ck " .. ck)
	luci.http.prepare_content("application/json")
	luci.http.write_json(output)
end
