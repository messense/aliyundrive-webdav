FROM alpine:latest
ARG TARGETARCH
RUN apk --no-cache add ca-certificates
WORKDIR /root/
ADD aliyundrive-webdav-$TARGETARCH ./aliyundrive-webdav
ENTRYPOINT ["/root/aliyundrive-webdav"]
