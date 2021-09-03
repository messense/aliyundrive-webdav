#!/bin/sh
source /koolshare/scripts/base.sh
MODULE=aliyundrivewebdav
DIR=$(cd $(dirname $0); pwd)

cd /tmp
killall aliyundrive-webdav
rm -f /koolshare/bin/aliyundrivewebdav.log
cp -rf /tmp/aliyundrivewebdav/bin/* /koolshare/bin/
cp -rf /tmp/aliyundrivewebdav/scripts/* /koolshare/scripts/
cp -rf /tmp/aliyundrivewebdav/webs/* /koolshare/webs/
cp -rf /tmp/aliyundrivewebdav/res/* /koolshare/res/

chmod a+x /koolshare/bin/aliyundrive-webdav
chmod a+x /koolshare/scripts/aliyundrivewebdav_config.sh
chmod a+x /koolshare/scripts/uninstall_aliyundrivewebdav.sh
ln -sf /koolshare/scripts/aliyundrivewebdav_config.sh /koolshare/init.d/S99aliyundrivewebdav.sh

dbus set softcenter_module_${MODULE}_name="${MODULE}"
dbus set softcenter_module_${MODULE}_title="阿里云盘WebDAV"
dbus set softcenter_module_${MODULE}_description="阿里云盘 WebDAV 服务器"
dbus set softcenter_module_${MODULE}_version="$(cat $DIR/version)"
dbus set softcenter_module_${MODULE}_install="1"

# 默认配置
dbus set ${MODULE}_port="8080"
dbus set ${MODULE}_read_buffer_size="10485760"
dbus set ${MODULE}_cache_size="1000"

rm -rf /tmp/aliyundrivewebdav* >/dev/null 2>&1
aw_enable=`dbus get aliyundrivewebdav_enable`
if [ "${aw_enable}"x = "1"x ];then
    /koolshare/scripts/aliyundrivewebdav_config.sh
fi
logger "[软件中心]: 完成 aliyundrivewebdav 安装"
exit
