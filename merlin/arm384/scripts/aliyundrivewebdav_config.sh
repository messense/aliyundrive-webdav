#!/bin/sh
eval `dbus export aliyundrivewebdav`
source /koolshare/scripts/base.sh
alias echo_date='echo $(date +%Y年%m月%d日\ %X):'
LOG_FILE=/tmp/upload/aliyundrivewebdavconfig.log
rm -rf $LOG_FILE
BIN=/koolshare/bin/aliyundrive-webdav
http_response "$1"

case $2 in
1)
    echo_date "当前已进入aliyundrivewebdav_config.sh" >> $LOG_FILE
    sh /koolshare/scripts/aliyundrivewebdavconfig.sh restart
    echo BBABBBBC >> $LOG_FILE
    ;;
esac
