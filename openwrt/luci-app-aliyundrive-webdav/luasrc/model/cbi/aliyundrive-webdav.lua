local a=require"luci.sys"
local e=luci.model.uci.cursor()
local e=require"nixio.fs"
require("luci.sys")
local t,e,o

t=Map("aliyundrive-webdav",translate("AliyunDriveWebDAV"))

t:section(SimpleSection).template = "aliyundrive-webdav-status"

e=t:section(TypedSection,"server",translate("WebDAV Server"))
e.anonymous=true

enable=e:option(Flag,"enable",translate("enable"))
enable.rmempty=false
host=e:option(Value,"host",translate("Host"))
host.default="0.0.0.0"
port=e:option(Value,"port",translate("Port"))
port.default="8080"
auth_user=e:option(Value,"auth_user",translate("Username"))
auth_password=e:option(Value,"auth_password",translate("Password"))
auth_password.password = true
read_buffer_size=e:option(Value,"read_buffer_size",translate("Read Buffer Size"))
read_buffer_size.default = "10485760"

e=t:section(TypedSection,"aliyun",translate("AliyunDrive"))
e.anonymous=true

refresh_token=e:option(Value,"refresh_token",translate("Refresh Token"))

local a="/var/log/aliyundrive-webdav.log"
tvlog=e:option(TextValue,"sylogtext")
tvlog.rows=16
tvlog.readonly="readonly"
tvlog.wrap="off"

function tvlog.cfgvalue(e,e)
	sylogtext=""
	if a and nixio.fs.access(a) then
		sylogtext=luci.sys.exec("tail -n 100 %s | sed 's/\\x1b\\[[0-9;]*m//g'"%a)
	end
	return sylogtext
end

tvlog.write=function(e,e,e)
end
local e=luci.http.formvalue("cbi.apply")
if e then
    io.popen("/etc/init.d/aliyundrive-webdav restart")
end
return t