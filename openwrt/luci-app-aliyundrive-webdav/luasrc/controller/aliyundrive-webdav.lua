module("luci.controller.aliyundrive-webdav",package.seeall)
function index()
entry({"admin","services","aliyundrive-webdav"},cbi("aliyundrive-webdav"),_("AliyunDriveWebDAV"),58).acl_depends = { "luci-app-aliyundrive-webdav" }
end