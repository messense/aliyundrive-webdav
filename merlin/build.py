#!/usr/bin/env python
# _*_ coding:utf-8 _*_
from __future__ import print_function

import os
import sys
import json
import codecs
import hashlib
import platform
from string import Template

parent_path = os.path.dirname(os.path.realpath(__file__))
module_name = "aliyundrivewebdav"


def md5sum(full_path):
    with open(full_path, 'rb') as rf:
        return hashlib.md5(rf.read()).hexdigest()


def get_or_create():
    conf_path = os.path.join(parent_path, "config.json.js")
    conf = {}
    if not os.path.isfile(conf_path):
        print("config.json.js not found，build.py is root path. auto write config.json.js")
        conf["module"] = module_name
        conf["version"] = "0.1.0"
        conf["home_url"] = ("Module_%s.asp" % module_name)
        conf["title"] = "title of " + module_name
        conf["description"] = "description of " + module_name
    else:
        with codecs.open(conf_path, "r", "utf-8") as fc:
            conf = json.loads(fc.read())
    return conf


def build_module():
    try:
        conf = get_or_create()
    except Exception:
        print("config.json.js file format is incorrect")
        return

    if len(sys.argv) != 2:
        print("Usage: python build.py <path>")
        return

    module_root = sys.argv[1]
    if "module" not in conf:
        print("module is not in config.json.js")
        return
    module_path = os.path.join(parent_path, module_root)
    if not os.path.isdir(module_path):
        print("%s dir not found，check config.json.js is module ?" % module_path)
        return
    install_path = os.path.join(module_path, "install.sh")
    if not os.path.isfile(install_path):
        print("%s file not found，check install.sh file" % install_path)
        return

    with codecs.open(os.path.join(module_path, "version"), "w", "utf-8") as fw:
        fw.write(conf["version"])

    print("build...")
    if platform.system() == "Darwin":
        t = Template("cd $parent_path && rm -f $module.tar.gz && tar -zcf $module.tar.gz -s /^$module_root/$module/ $module_root")
    else:
        t = Template("cd $parent_path && rm -f $module.tar.gz && tar -zcf $module.tar.gz --transform s/$module_root/$module/ $module_root")
    os.system(t.substitute({
        "parent_path": parent_path,
        "module": conf["module"],
        "module_root": module_root,
    }))
    conf["md5"] = md5sum(os.path.join(parent_path, conf["module"] + ".tar.gz"))
    conf_path = os.path.join(parent_path, "config.json.js")
    with codecs.open(conf_path, "w", "utf-8") as fw:
        json.dump(conf, fw, sort_keys=True, indent=4, ensure_ascii=False)
    print("build done", conf["module"] + ".tar.gz")
    hook_path = os.path.join(parent_path, "backup.sh")
    if os.path.isfile(hook_path):
        os.system(hook_path)


if __name__ == '__main__':
    build_module()
