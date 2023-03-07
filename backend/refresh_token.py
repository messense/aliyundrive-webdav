import time

import requests
import streamlit as st


st.title("aliyundrive-webdav")
st.header("refresh token 获取工具")

if st.button("点击生成二维码"):
    res = requests.post(
        "https://aliyundrive-oauth.messense.me/oauth/authorize/qrcode",
        json={
            "scopes": ["user:base", "file:all:read", "file:all:write"],
            "width": 300,
            "height": 300,
        },
    )
    res.raise_for_status()
    data = res.json()
    sid = data["sid"]
    qrcode_url = data["qrCodeUrl"]
    st.image(qrcode_url, caption="使用阿里云盘 App 扫码")

    while True:
        res = requests.get(f"https://openapi.aliyundrive.com/oauth/qrcode/{sid}/status")
        res.raise_for_status()
        data = res.json()
        status = data["status"]
        if status == "LoginSuccess":
            code = data["authCode"]
            res = requests.post(
                "https://aliyundrive-oauth.messense.me/oauth/access_token",
                json={
                    "grant_type": "authorization_code",
                    "code": code,
                },
            )
            res.raise_for_status()
            data = res.json()
            refresh_token = data["refresh_token"]
            st.success("refresh token 获取成功", icon="✅")
            st.code(refresh_token, language=None)
            break

        time.sleep(1)
