import asyncio

import httpx
import streamlit as st
from streamlit_extras.stylable_container import stylable_container


session = httpx.AsyncClient()


async def get_qrcode_status(sid: str) -> dict:
    res = await session.get(
        f"https://openapi.aliyundrive.com/oauth/qrcode/{sid}/status"
    )
    return res.json()


async def get_refresh_token(code: str) -> str:
    res = await session.post(
        "https://aliyundrive-oauth.messense.me/oauth/access_token",
        json={
            "grant_type": "authorization_code",
            "code": code,
        },
    )
    data = res.json()
    refresh_token = data["refresh_token"]
    return refresh_token


async def main():
    st.set_page_config(
        page_title="aliyundrive-webdav refresh token 获取工具",
        layout="wide",
    )
    st.title("aliyundrive-webdav refresh token 获取")
    st.markdown(
        "👏 欢迎加入 [aliyundrive-webdav 知识星球](https://t.zsxq.com/0c9sq6Ca8)获取咨询和技术支持服务"
    )

    qrcode_tab, authcode_tab = st.tabs(["扫码授权", "authCode"])

    with qrcode_tab:
        if st.button("点击获取扫码登录二维码"):
            res = await session.post(
                "https://aliyundrive-oauth.messense.me/oauth/authorize/qrcode",
                json={
                    "scopes": ["user:base", "file:all:read", "file:all:write"],
                    "width": 300,
                    "height": 300,
                },
            )
            data = res.json()
            sid = data["sid"]
            qrcode_url = data["qrCodeUrl"]
            st.image(qrcode_url, caption="使用阿里云盘 App 扫码")

            refresh_token = None
            with st.spinner("等待扫码授权中..."):
                while True:
                    try:
                        data = await get_qrcode_status(sid)
                    except httpx.ConnectTimeout:
                        st.error(
                            "查询扫码结果超时, 可能是触发了阿里云盘接口限制, 请稍后再试.\n"
                            "或者自行尝试轮询此接口后切换到 authCode tab 获取 refresh token: "
                            f"https://openapi.aliyundrive.com/oauth/qrcode/{sid}/status",
                            icon="🚨",
                        )
                        break

                    status = data["status"]
                    if status == "LoginSuccess":
                        code = data["authCode"]
                        refresh_token = await get_refresh_token(code)
                        break
                    elif status == "QRCodeExpired":
                        st.error("二维码已过期, 请刷新页面后重试", icon="🚨")
                        break

                    await asyncio.sleep(2)

            if refresh_token:
                st.success("refresh token 获取成功", icon="✅")
                with stylable_container(
                    "codeblock",
                    "code { white-space: normal !important; overflow-wrap: anywhere; }",
                ):
                    st.code(refresh_token, language=None)

    with authcode_tab:
        with st.form("authCode"):
            code = st.text_input("authCode", help="填入 authCode")
            submitted = st.form_submit_button("提交")
            if submitted and code:
                try:
                    refresh_token = await get_refresh_token(code)
                    st.success("refresh token 获取成功", icon="✅")
                    with stylable_container(
                        "codeblock",
                        "code { white-space: normal !important; overflow-wrap: anywhere; }",
                    ):
                        st.code(refresh_token, language=None)
                except KeyError:
                    st.error("无效的 authCode, 请重新获取", icon="🚨")


if __name__ == "__main__":
    try:
        import uvloop
    except ImportError:
        pass
    else:
        uvloop.install()

    asyncio.run(main())
