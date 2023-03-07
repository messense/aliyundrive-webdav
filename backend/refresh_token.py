import asyncio

import httpx
import streamlit as st


session = httpx.AsyncClient()


async def main():
    st.set_page_config(
        page_title="aliyundrive-webdav refresh token 获取工具",
    )
    st.title("aliyundrive-webdav")
    st.header("refresh token 获取工具")

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
                res = await session.get(
                    f"https://openapi.aliyundrive.com/oauth/qrcode/{sid}/status"
                )
                data = res.json()
                status = data["status"]
                if status == "LoginSuccess":
                    code = data["authCode"]
                    res = await session.post(
                        "https://aliyundrive-oauth.messense.me/oauth/access_token",
                        json={
                            "grant_type": "authorization_code",
                            "code": code,
                        },
                    )
                    data = res.json()
                    refresh_token = data["refresh_token"]
                    break
                elif status == "QRCodeExpired":
                    st.error("二维码已过期, 请刷新页面后重试", icon="🚨")
                    break

                await asyncio.sleep(2)

        if refresh_token:
            st.success("refresh token 获取成功", icon="✅")
            st.code(refresh_token, language=None)


if __name__ == "__main__":
    try:
        import uvloop
    except ImportError:
        pass
    else:
        uvloop.install()

    asyncio.run(main())
