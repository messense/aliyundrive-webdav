FROM alpine:latest
ARG TARGETARCH
ARG TARGETVARIANT
RUN apk --no-cache add ca-certificates tini
RUN apk add tzdata && \
	cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
	echo "Asia/Shanghai" > /etc/timezone && \
	apk del tzdata

RUN mkdir -p /etc/aliyundrive-webdav
WORKDIR /root/
ADD aliyundrive-webdav-$TARGETARCH$TARGETVARIANT /usr/bin/aliyundrive-webdav

ENV NO_SELF_UPGRADE 1

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["/usr/bin/aliyundrive-webdav", "--auto-index", "--workdir", "/etc/aliyundrive-webdav"]
