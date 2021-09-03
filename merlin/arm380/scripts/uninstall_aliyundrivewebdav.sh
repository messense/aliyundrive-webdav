#!/bin/sh
eval `dbus export aliyundrivewebdav_`
source /koolshare/scripts/base.sh
logger "[软件中心]: 正在卸载 aliyundrivewebdav..."
MODULE=aliyundrivewebdav
cd /
/koolshare/scripts/aliyundrivewebdav_config.sh stop
rm -f /koolshare/init.d/S99aliyundrivewebdav.sh
rm -f /koolshare/scripts/aliyundrivewebdav_config.sh
rm -f /koolshare/webs/Module_aliyundrivewebdav.asp
rm -f /koolshare/res/icon-aliyundrivewebdav.png
rm -f /koolshare/bin/aliyundrive-webdav
rm -f /koolshare/bin/aliyundrivewebdav.log /tmp/aliyundrivewebdav.log
rm -f /tmp/var/run/aliyundrivewebdav.pid
rm -fr /tmp/aliyundrivewebdav* >/dev/null 2>&1
dbus remove __event__onnatstart_aliyundrivewebdav
values=`dbus list aliyundrivewebdav | cut -d "=" -f 1`
for value in $values
do
  dbus remove $value
done
logger "[软件中心]: 完成 aliyundrivewebdav 卸载"
rm -f /koolshare/scripts/uninstall_aliyundrivewebdav.sh
