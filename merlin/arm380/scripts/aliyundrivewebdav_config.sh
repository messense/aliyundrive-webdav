#!/bin/sh
eval `dbus export aliyundrivewebdav`
source /koolshare/scripts/base.sh
alias echo_date='echo $(date +%Y年%m月%d日\ %X):'

BIN=/koolshare/bin/aliyundrive-webdav
PID_FILE=/var/run/aliyundrivewebdav.pid

if [ "$(cat /proc/sys/vm/overcommit_memory)"x != "0"x ];then
    echo 0 > /proc/sys/vm/overcommit_memory
fi

aliyundrivewebdav_start_stop(){
    if [ "${aliyundrivewebdav_enable}"x = "1"x ];then
        killall aliyundrive-webdav
        AUTH_ARGS=""
        if [ "${aliyundrivewebdav_auth_user}"x != ""x ];then
          AUTH_ARGS="--auth-user ${aliyundrivewebdav_auth_user}"
        fi
        if [ "${aliyundrivewebdav_auth_password}"x != ""x ];then
          AUTH_ARGS="$AUTH_ARGS --auth-password ${aliyundrivewebdav_auth_password}"
        fi
        if [ "${aliyundrivewebdav_read_buffer_size}"x = ""x ];then
          aliyundrivewebdav_read_buffer_size="10485760"
        fi
        if [ "${aliyundrivewebdav_cache_size}"x = ""x ];then
          aliyundrivewebdav_cache_size="1000"
        fi
        if [ "${aliyundrivewebdav_root}"x = ""x ];then
          aliyundrivewebdav_root="/"
        fi

        start-stop-daemon -S -q -b -m -p ${PID_FILE} \
          -x /bin/sh -- -c "${BIN} -I --host 0.0.0.0 -p ${aliyundrivewebdav_port} -r ${aliyundrivewebdav_refresh_token} --root ${aliyundrivewebdav_root} --workdir /var/run/aliyundrivewebdav -S ${aliyundrivewebdav_read_buffer_size} --cache-size ${aliyundrivewebdav_cache_size} $AUTH_ARGS >/tmp/aliyundrivewebdav.log 2>&1"
    else
        killall aliyundrive-webdav
    fi
}

aliyundrivewebdav_nat_start(){
    if [ "${aliyundrivewebdav_enable}"x = "1"x ];then
        echo_date 添加nat-start触发事件...
        dbus set __event__onnatstart_aliyundrivewebdav="/koolshare/scripts/aliyundrivewebdav_config.sh"
    else
        echo_date 删除nat-start触发...
        dbus remove __event__onnatstart_aliyundrivewebdav
    fi
}

case ${ACTION} in
start)
    aliyundrivewebdav_start_stop
    aliyundrivewebdav_nat_start
    ;;
*)
    aliyundrivewebdav_start_stop
    aliyundrivewebdav_nat_start
    ;;
esac
