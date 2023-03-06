from __future__ import annotations
import os

import httpx
from fastapi import FastAPI, Response
from pydantic import BaseModel

CLIENT_ID = os.getenv("ALIYUNDRIVE_CLIENT_ID")
CLIENT_SECRET = os.getenv("ALIYUNDRIVE_CLIENT_SECRET")

app = FastAPI()


class QrCodeRequest(BaseModel):
    scopes: list[str]
    width: int | None = None
    height: int | None = None


class AuthorizationRequest(BaseModel):
    grant_type: str
    code: str | None = None
    refresh_token: str | None = None


@app.post("/oauth/authorize/qrcode")
async def qrcode(request: QrCodeRequest) -> Response:
    async with httpx.AsyncClient() as client:
        res = await client.post(
            "https://openapi.aliyundrive.com/oauth/authorize/qrcode",
            json={
                "client_id": CLIENT_ID,
                "client_secret": CLIENT_SECRET,
                "scopes": request.scopes,
                "width": request.width,
                "height": request.height,
            },
        )
        return Response(
            content=res.content,
            status_code=res.status_code,
            media_type=res.headers["Content-Type"],
        )


@app.post("/oauth/access_token")
async def access_token(request: AuthorizationRequest) -> Response:
    async with httpx.AsyncClient() as client:
        res = await client.post(
            "https://openapi.aliyundrive.com/oauth/access_token",
            json={
                "client_id": CLIENT_ID,
                "client_secret": CLIENT_SECRET,
                "grant_type": request.grant_type,
                "code": request.code,
                "refresh_token": request.refresh_token,
            },
        )
        return Response(
            content=res.content,
            status_code=res.status_code,
            media_type=res.headers["Content-Type"],
        )
