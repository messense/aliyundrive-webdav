module("luci.controller.aliyundrive-webdav",package.seeall)

local fs = require "nixio.fs"
local http = require "luci.http"
local uci = require"luci.model.uci".cursor()

function index()
    entry({"admin","services","aliyundrive-webdav"},cbi("aliyundrive-webdav"),_("AliyunDriveWebDAV"),58).acl_depends = { "luci-app-aliyundrive-webdav" }
    entry({"admin", "services", "aliyundrive-webdav", "status"}, call("adw_status")).leaf = true
end

function adw_status()
    local e = {}
    local binpath = "/usr/bin/aliyundrive-webdav"
    e.running = luci.sys.call("pgrep " .. binpath .. " >/dev/null") == 0
    http.prepare_content("application/json")
    http.write_json(e)
end