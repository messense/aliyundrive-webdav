local uci = luci.model.uci.cursor()
local m, e

m = Map("aliyundrive-webdav", translate("AliyunDriveWebDAV"))

m:section(SimpleSection).template = "aliyundrive-webdav/status"

e = m:section(TypedSection, "server")
e.anonymous = true

enable = e:option(Flag, "enable", translate("enable"))
enable.rmempty = false
refresh_token = e:option(Value, "refresh_token", translate("Refresh Token"))
host = e:option(Value, "host", translate("Host"))
host.default = "0.0.0.0"
host.datatype = "ipaddr"
port = e:option(Value, "port", translate("Port"))
port.default = "8080"
port.datatype = "port"
auth_user = e:option(Value, "auth_user", translate("Username"))
auth_password = e:option(Value, "auth_password", translate("Password"))
auth_password.password = true
read_buffer_size = e:option(Value, "read_buffer_size", translate("Read Buffer Size"))
read_buffer_size.default = "10485760"
read_buffer_size.datatype = "uinteger"
cache_size = e:option(Value, "cache_size", translate("Cache Size"))
cache_size.default = "1000"
cache_size.datatype = "uinteger"
debug = e:option(Flag, "debug", translate("Debug Mode"))
debug.rmempty = false

local e = luci.http.formvalue("cbi.apply")
if e then
    io.popen("/etc/init.d/aliyundrive-webdav restart")
end
return m