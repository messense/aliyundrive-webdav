<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Transitional//EN" "http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<html xmlns:v>
    <head>
        <meta http-equiv="X-UA-Compatible" content="IE=Edge" />
        <meta http-equiv="Content-Type" content="text/html; charset=utf-8" />
        <meta http-equiv="Pragma" content="no-cache" />
        <meta http-equiv="Expires" content="-1" />
        <link rel="shortcut icon" href="images/favicon.png" />
        <link rel="icon" href="images/favicon.png" />
        <title>阿里云盘 WebDAV</title>
        <link rel="stylesheet" type="text/css" href="index_style.css">
        <link rel="stylesheet" type="text/css" href="form_style.css">
        <link rel="stylesheet" type="text/css" href="usp_style.css">
        <link rel="stylesheet" type="text/css" href="css/element.css">
        <link rel="stylesheet" type="text/css" href="/device-map/device-map.css">
        <link rel="stylesheet" type="text/css" href="/js/table/table.css">
        <link rel="stylesheet" type="text/css" href="/res/layer/theme/default/layer.css">
        <link rel="stylesheet" type="text/css" href="/res/softcenter.css">
        <link rel="stylesheet" type="text/css" href="/res/aliyundrivewebdav.css">
        <script type="text/javascript" src="/state.js"></script>
        <script type="text/javascript" src="/popup.js"></script>
        <script type="text/javascript" src="/help.js"></script>
        <script type="text/javascript" src="/js/jquery.js"></script>
        <script type="text/javascript" src="/general.js"></script>
        <script type="text/javascript" language="JavaScript" src="/js/table/table.js"></script>
        <script type="text/javascript" language="JavaScript" src="/client_function.js"></script>
        <script type="text/javascript" src="/res/aliyundrivewebdav-menu.js"></script>
        <script type="text/javascript" src="/res/softcenter.js"></script>
        <script type="text/javascript" src="/help.js"></script>
        <script type="text/javascript" src="/switcherplugin/jquery.iphone-switch.js"></script>
        <script type="text/javascript" src="/validator.js"></script>
        <script>
            var db_aliyundrivewebdav={};
            var _responseLen;
            var noChange = 0;
            var x = 5;
            var params_inputs = ['aliyundrivewebdav_refresh_token', 'aliyundrivewebdav_port', 'aliyundrivewebdav_auth_user', 'aliyundrivewebdav_auth_password', 'aliyundrivewebdav_read_buffer_size', 'aliyundrivewebdav_cache_size', 'aliyundrivewebdav_root'];
            var params_check = ['aliyundrivewebdav_enable', 'aliyundrivewebdav_public'];
            function init() {
                show_menu(menu_hook);
                get_dbus_data();
                buildswitch();
                update_visibility();
                get_status_front();
            }
            function get_dbus_data(){
                $.ajax({
                    type: "GET",
                    url: "/_api/aliyundrivewebdav_",
                    async: false,
                    success: function(data) {
                        db_aliyundrivewebdav = data.result[0];
                        conf2obj();
                       }
                });
            }
            function conf2obj() {
                for (var i = 0; i < params_check.length; i++) {
                    console.log(params_check.length);
                    E(params_check[i]).checked = db_aliyundrivewebdav[params_check[i]] == 1 ? true : false
                }
                for (var i = 0; i < params_inputs.length; i++) {
                    if (db_aliyundrivewebdav[params_inputs[i]]) {
                        E(params_inputs[i]).value = db_aliyundrivewebdav[params_inputs[i]];
                    }
                }
            }
            function reload_Soft_Center(){
                location.href = "/Module_Softcenter.asp";
            }

            function buildswitch(){
                $("#aliyundrivewebdav_enable").click(
                function(){
                    update_visibility();
                });
            }

            function update_visibility(){
                if (document.getElementById('aliyundrivewebdav_enable').checked) {
                    document.getElementById("state_tr").style.display = "";
                    document.getElementById("refresh_token_tr").style.display = "";
                    document.getElementById("root_tr").style.display = "";
                    document.getElementById("port_tr").style.display = "";
                    document.getElementById("auth_user_tr").style.display = "";
                    document.getElementById("auth_password_tr").style.display = "";
                    document.getElementById("read_buffer_size_tr").style.display = "";
                    document.getElementById("cache_size_tr").style.display = "";
                    document.getElementById("public_table").style.display = "";
                } else {
                    document.getElementById("state_tr").style.display = "none";
                    document.getElementById("refresh_token_tr").style.display = "none";
                    document.getElementById("root_tr").style.display = "none";
                    document.getElementById("port_tr").style.display = "none";
                    document.getElementById("auth_user_tr").style.display = "none";
                    document.getElementById("auth_password_tr").style.display = "none";
                    document.getElementById("read_buffer_size_tr").style.display = "none";
                    document.getElementById("cache_size_tr").style.display = "none";
                    document.getElementById("public_table").style.display = "none";
                }

            }
            function get_status_front() {
                if (db_aliyundrivewebdav['aliyundrivewebdav_enable'] != "1") {
                    E("aliyundrivewebdav_state1").innerHTML = "运行状态 " + "Waiting...";
                    return false;
                }
                var id = parseInt(Math.random() * 100000000);
                var postData = {"id": id, "method": "aliyundrivewebdav_status.sh", "params":[], "fields": ""};
                $.ajax({
                    type: "POST",
                    url: "/_api/",
                    async: true,
                    data: JSON.stringify(postData),
                    success: function(response) {
                        var arr = response.result.split("@");
                        if (arr[0] == "") {
                            E("aliyundrivewebdav_state1").innerHTML = "aliyudrive-webdav 启动时间 - " + "Waiting for first refresh...";
                        } else {
                            E("aliyundrivewebdav_state1").innerHTML = arr[0];
                            E("aliyundrivewebdav_version").innerHTML = arr[1];
                        }
                    }
                });
                setTimeout("get_status_front();", 60000);
            }
            function save(){
                if(!$.trim($('#aliyundrivewebdav_refresh_token').val())){
                    alert("refresh token不能为空！！！");
                    return false;
                }
                if(!$.trim($('#aliyundrivewebdav_port').val())){
                    alert("监听端口不能为空！！！");
                    return false;
                }
                if(!$.trim($('#aliyundrivewebdav_read_buffer_size').val())){
                    alert("下载缓冲大小不能为空！！！");
                    return false;
                }
                if(!$.trim($('#aliyundrivewebdav_cache_size').val())){
                    alert("目录缓存不能为空！！！");
                    return false;
                }

                for (var i = 0; i < params_check.length; i++) {
                    db_aliyundrivewebdav[params_check[i]] = E(params_check[i]).checked ? '1' : '0';
                }
                //input
                for (var i = 0; i < params_inputs.length; i++) {
                    if (E(params_inputs[i]).value) {
                        db_aliyundrivewebdav[params_inputs[i]] = E(params_inputs[i]).value;
                    }
                }
                db_aliyundrivewebdav["aliyundrivewebdav_action"] = action = 1;
	            push_data("aliyundrivewebdav_config.sh", action,  db_aliyundrivewebdav);
           }
           function push_data(script, arg, obj, flag){
                if (!flag) showALIDRIVELoadingBar();
                var id = parseInt(Math.random() * 100000000);
                var postData = {"id": id, "method": script, "params":[arg], "fields": obj};
                $.ajax({
                    type: "POST",
                    cache:false,
                    url: "/_api/",
                    data: JSON.stringify(postData),
                    dataType: "json",
                    success: function(response){
                        if(response.result == id){
                            if(flag && flag == "1"){
                                refreshpage();
                            }else if(flag && flag == "2"){
                                //continue;
                                //do nothing
                            }else{
                                get_realtime_log();
                            }
                        }
                    }
                });
            }
            function get_realtime_log() {
                $.ajax({
                    url: '/_temp/aliyundrivewebdavconfig.log',
                    type: 'GET',
                    async: true,
                    cache: false,
                    dataType: 'text',
                    success: function(response) {
                        var retArea = E("log_content3");
                        if (response.search("BBABBBBC") != -1) {
                            retArea.value = response.replace("BBABBBBC", " ");
                            E("ok_button").style.display = "";
                            retArea.scrollTop = retArea.scrollHeight;
                            count_down_close1();
                            return true;
                        }
                        if (_responseLen == response.length) {
                            noChange++;
                        } else {
                            noChange = 0;
                        }
                        if (noChange > 1000) {
                            return false;
                        } else {
                            setTimeout("get_realtime_log();", 300);
                        }
                        retArea.value = response.replace("BBABBBBC", " ");
                        retArea.scrollTop = retArea.scrollHeight;
                        _responseLen = response.length;
                    },
                    error: function() {
                        setTimeout("get_realtime_log();", 500);
                    }
                });
            }
            function count_down_close1() {
                if (x == "0") {
                    hideALIDRIVELoadingBar();
                }
                if (x < 0) {
                    E("ok_button1").value = "手动关闭"
                    return false;
                }
                E("ok_button1").value = "自动关闭（" + x + "）"
                    --x;
                setTimeout("count_down_close1();", 1000);
            }
        </script>
    </head>
    <body onload="init();">
        <div id="TopBanner"></div>
        <div id="Loading" class="popup_bg"></div>
        <div id="LoadingBar" class="popup_bar_bg_ks" style="z-index: 200;" >
            <table cellpadding="5" cellspacing="0" id="loadingBarBlock" class="loadingBarBlock" align="center">
                <tr>
                    <td height="100">
                    <div id="loading_block3" style="margin:10px auto;margin-left:10px;width:85%; font-size:12pt;"></div>
                    <div id="loading_block2" style="margin:10px auto;width:95%;"></div>
                    <div id="log_content2" style="margin-left:15px;margin-right:15px;margin-top:10px;overflow:hidden">
                        <textarea cols="50" rows="36" wrap="off" readonly="readonly" id="log_content3" autocomplete="off" autocorrect="off" autocapitalize="off" spellcheck="false" style="border:1px solid #000;width:99%; font-family:'Lucida Console'; font-size:11px;background:transparent;color:#FFFFFF;outline: none;padding-left:3px;padding-right:22px;overflow-x:hidden"></textarea>
                    </div>
                    <div id="ok_button" class="apply_gen" style="background: #000;display: none;">
                        <input id="ok_button1" class="button_gen" type="button" onclick="hideALIDRIVELoadingBar()" value="确定">
                    </div>
                    </td>
                </tr>
            </table>
        </div>
        <table class="content" align="center" cellpadding="0" cellspacing="0">
            <tbody>
                <tr>
                    <td width="17">&nbsp;</td>
                    <td valign="top" width="202">
                        <div id="mainMenu"></div>
                        <div id="subMenu"></div>
                    </td>
                    <td valign="top">
                        <div id="tabMenu" class="submenuBlock"></div>
                        <!--=====Beginning of Main Content=====-->
                        <table width="98%" border="0" align="left" cellpadding="0" cellspacing="0" style="display: block;">
                            <tbody>
                                <tr>
                                    <td align="left" valign="top">
                                        <div>
                                            <table width="760px" border="0" cellpadding="5" cellspacing="0" bordercolor="#6b8fa3" class="FormTitle" id="FormTitle">
                                                <tbody>
                                                    <tr>
                                                        <td bgcolor="#4D595D" colspan="3" valign="top">
                                                            <div>&nbsp;</div>
										<div class="formfonttitle">阿里云盘WebDAV - 设置</div>
										<div style="float:right; width:15px; height:25px;margin-top:-20px">
											<img id="return_btn" onclick="reload_Soft_Center();" align="right" style="cursor:pointer;position:absolute;margin-left:-30px;margin-top:-25px;" title="返回软件中心" src="/images/backprev.png" onMouseOver="this.src='/images/backprevclick.png'" onMouseOut="this.src='/images/backprev.png'"></img>
										</div>
										<div style="margin:10px 0 10px 5px;" class="splitLine"></div>
										<div class="SimpleNote" id="head_illustrate"><i></i>
											<p>阿里云盘 refresh token 可以在登录<a href="https://www.aliyundrive.com/drive/" target="_blank">阿里云盘网页版</a>后在开发者工具 -&gt; Application -&gt; Local Storage 中的 token 字段中找到</p>
										</div><div>&nbsp;</div>
                                                            <table style="margin:20px 0px 0px 0px;" width="100%" border="1" align="center" cellpadding="4" cellspacing="0" bordercolor="#6b8fa3" class="FormTable">
                                                                <thead>
                                                                    <tr>
                                                                        <td colspan="2">阿里云盘 WebDAV - 设置面板</td>
                                                                    </tr>
                                                                </thead>
                                                                <tbody>
                                                                    <tr id="switch_tr">
                                                                        <th> <label>开启阿里云盘 WebDAV</label> </th>
                                                                        <td colspan="2">
                                                                            <div class="switch_field" style="display:table-cell">
                                                                                <label for="aliyundrivewebdav_enable">
                                                                                    <input id="aliyundrivewebdav_enable" class="switch" type="checkbox" style="display: none;">
                                                                                    <div class="switch_container">
                                                                                        <div class="switch_bar"></div>
                                                                                        <div class="switch_circle transition_style">
                                                                                            <div></div>
                                                                                        </div>
                                                                                    </div>
                                                                                </label>
                                                                            </div>
                                                                        </td>
                                                                    </tr>
                                                                    <tr id="state_tr">
                                                                        <th>运行状态</th>
                                                                            <td>
                                                                                <div style="display:table-cell;float: left;margin-left:0px;">
                                                                                        <span id="aliyundrivewebdav_state1">运行状态 - Waiting...</span>
                                                                                </div>
                                                                            </td>
                                                                    </tr>
                                                                    <tr>
                                                                        <th >插件版本</th>
                                                                        <td colspan="2"  id="aliyundrivewebdav_version">
                                                                        </td>
                                                                    </tr>
                                                                    <tr id="refresh_token_tr">
                                                                        <th>refresh token</th>
                                                                        <td> <input type="text" id="aliyundrivewebdav_refresh_token" class="input_15_table" value=""></td>
                                                                    </tr>
                                                                    <tr id="root_tr">
                                                                        <th>根目录</th>
                                                                        <td> <input type="text" id="aliyundrivewebdav_root" class="input_15_table" value=""></td>
                                                                    </tr>
                                                                    <tr id="port_tr">
                                                                        <th>监听端口</th>
                                                                        <td><input type="text" id="aliyundrivewebdav_port" class="input_15_table" value=""></td>
                                                                    </tr>
                                                                    <tr id="auth_user_tr">
                                                                        <th>用户名</th>
                                                                        <td><input type="text" id="aliyundrivewebdav_auth_user" class="input_15_table" value=""></td>
                                                                    </tr>
                                                                    <tr id="auth_password_tr">
                                                                        <th>密码</th>
                                                                        <td><input type="text" id="aliyundrivewebdav_auth_password" class="input_15_table" value=""></td>
                                                                    </tr>
                                                                    <tr id="read_buffer_size_tr">
                                                                        <th>下载缓冲大小(bytes)</th>
                                                                        <td><input type="text" id="aliyundrivewebdav_read_buffer_size" value="10485760" class="input_15_table"></td>
                                                                    </tr>
                                                                    <tr id="cache_size_tr">
                                                                        <th>目录缓存大小</th>
                                                                        <td><input type="text" id="aliyundrivewebdav_cache_size" value="1000" class="input_15_table"></td>
                                                                    </tr>
                                                                </tbody>
                                                            </table>
                                                            <table id="public_table" style="margin:10px 0px 0px 0px;" width="100%" border="1" align="center" cellpadding="4" cellspacing="0" bordercolor="#6b8fa3" class="FormTable">
                                                                <thead>
                                                                <tr>
                                                                    <td colspan="2">公网访问设定 -- <em style="color: gold;">【建议设置五位数端口】【设置<a href="./Advanced_VirtualServer_Content.asp" target="_blank"><em>端口转发</em></a>】</em>【<a href="http://coolaf.com/tool/port" target="_blank"><em>检测端口开放情况</em></a>】</em></td>
                                                                </tr>
                                                                </thead>
                                                                <tr id="public">	
                                                                <th>开启公网访问</th>
                                                                <td colspan="2">
                                                                    <div class="switch_field" style="display:table-cell;float: left;">
                                                                    <label for="aliyundrivewebdav_public">
                                                                        <input id="aliyundrivewebdav_public" type="checkbox" name="public" class="switch" style="display: none;">
                                                                        <div class="switch_container" >
                                                                            <div class="switch_bar"></div>
                                                                            <div class="switch_circle transition_style">
                                                                                <div></div>
                                                                            </div>
                                                                        </div>
                                                                    </label>													
                                                                </div>
                                                                </td>
                                                                </tr>												
                                                            </table>
                                                            <div class="apply_gen">
                                                                <input class="button_gen" type="button" onclick="save()" value="提交" />
                                                            </div>
                                                            <div class="KoolshareBottom" style="margin-top:540px;">
                                                                论坛技术支持：
                                                                <a href="http://www.koolshare.cn" target="_blank"> <i><u>www.koolshare.cn</u></i> </a><br/>
                                                                Github项目：
                                                                <a href="https://github.com/koolshare/koolshare.github.io/tree/acelan_softcenter_ui" target="_blank"> <i><u>github.com/koolshare</u></i> </a><br />
                                                            </div>
                                                        </td>
                                                    </tr>
                                                </tbody>
                                            </table>
                                        </div>
                                    </td>
                                </tr>
                            </tbody>
                        </table>
                        <!--=====end of Main Content=====-->
                    </td>
                </tr>
            </tbody>
        </table>
        <div id="footer"></div>
    </body>
