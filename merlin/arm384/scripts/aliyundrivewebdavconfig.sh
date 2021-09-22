#!/bin/sh
eval `dbus export aliyundrivewebdav`
source /koolshare/scripts/base.sh
alias echo_date='echo $(date +%Y年%m月%d日\ %X):'
LOG_FILE=/tmp/upload/aliyundrivewebdavconfig.log
BIN=/koolshare/bin/aliyundrive-webdav

if [ "$(cat /proc/sys/vm/overcommit_memory)"x != "0"x ];then
    echo 0 > /proc/sys/vm/overcommit_memory
fi

aliyundrivewebdav_start_stop(){
    echo_date "当前已进入aliyundrivewebdavconfig.sh执行" >> $LOG_FILE
    if [ "${aliyundrivewebdav_enable}"x = "1"x ];then
        echo_date "先结束进程" >> $LOG_FILE
        killall aliyundrive-webdav
        AUTH_ARGS=""
        if [ "${aliyundrivewebdav_auth_user}"x != ""x ];then
          AUTH_ARGS="--auth-user ${aliyundrivewebdav_auth_user}"
        fi
        if [ "${aliyundrivewebdav_auth_password}"x != ""x ];then
          AUTH_ARGS="$AUTH_ARGS --auth-password ${aliyundrivewebdav_auth_password}"
        fi
        if [ "${aliyundrivewebdav_read_bufffer_size}"x = ""x ];then
          aliyundrivewebdav_read_bufffer_size="10485760"
        fi
        if [ "${aliyundrivewebdav_cache_size}"x = ""x ];then
          aliyundrivewebdav_cache_size="1000"
        fi
        if [ "${aliyundrivewebdav_root}"x = ""x ];then
          aliyundrivewebdav_root="/"
        fi
        echo_date "参数为：${aliyundrivewebdav_port} -r ${aliyundrivewebdav_refresh_token} --root ${aliyundrivewebdav_root} -S ${aliyundrivewebdav_read_buffer_size} --cache-size ${aliyundrivewebdav_cache_size} $AUTH_ARGS" >> $LOG_FILE
        #start-stop-daemon -S -q -b -m -p ${PID_FILE} \
        #  -x /bin/sh -- -c "${BIN} -I --workdir /var/run/aliyundrivewebdav --host 0.0.0.0 -p ${aliyundrivewebdav_port} -r ${aliyundrivewebdav_refresh_token} --root ${aliyundrivewebdav_root} -S ${aliyundrivewebdav_read_bufffer_size} $AUTH_ARGS >/tmp/aliyundrivewebdav.log 2>&1"
        ${BIN} -I --workdir /var/run/aliyundrivewebdav --host 0.0.0.0 -p ${aliyundrivewebdav_port} -r ${aliyundrivewebdav_refresh_token} --root ${aliyundrivewebdav_root} -S ${aliyundrivewebdav_read_buffer_size} --cache-size ${aliyundrivewebdav_cache_size} $AUTH_ARGS >/tmp/upload/aliyundrivewebdav.log 2>&1 &
        sleep 5s
        if [ ! -z "$(pidof aliyundrive-webdav)" -a ! -n "$(grep "Error" /tmp/upload/aliyundrivewebdav.log)" ] ; then
          echo_date "aliyundrive 进程启动成功！(PID: $(pidof aliyundrive-webdav))" >> $LOG_FILE
          if [ "$aliyundrivewebdav_public" == "1" ]; then
            iptables -I INPUT -p tcp --dport $aliyundrivewebdav_port -j ACCEPT >/dev/null 2>&1 &
          else
            iptables -D INPUT -p tcp --dport $aliyundrivewebdav_port -j ACCEPT >/dev/null 2>&1 &
          fi
        else
          echo_date "aliyundrive 进程启动失败！请检查参数是否存在问题，即将关闭" >> $LOG_FILE
          echo_date "失败原因：" >> $LOG_FILE
          error1=$(cat /tmp/upload/aliyundrivewebdav.log | grep -ioE "Error.*")
          if [ -n "$error1" ]; then
              echo_date $error1 >> $LOG_FILE
          fi
          dbus set aliyundrivewebdav_enable="0"
        fi
    else
        killall aliyundrive-webdav
        iptables -D INPUT -p tcp --dport $aliyundrivewebdav_port -j ACCEPT >/dev/null 2>&1 &
    fi
}
aliyundrivewebdav_stop(){
  killall aliyundrive-webdav
  iptables -D INPUT -p tcp --dport $aliyundrivewebdav_port -j ACCEPT >/dev/null 2>&1 &
}


case $ACTION in
start)
    aliyundrivewebdav_start_stop
    echo BBABBBBC >> $LOG_FILE
    ;;
start_nat)
    aliyundrivewebdav_start_stop
    echo BBABBBBC >> $LOG_FILE
    ;;
restart)
    aliyundrivewebdav_start_stop
    ;;
stop)
    aliyundrivewebdav_stop
    echo BBABBBBC >> $LOG_FILE
    ;;
*)
    aliyundrivewebdav_start_stop
    echo BBABBBBC >> $LOG_FILE
    ;;
esac
