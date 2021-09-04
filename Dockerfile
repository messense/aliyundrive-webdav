FROM alpine:latest
ARG TARGETARCH
ARG TARGETVARIANT
RUN apk --no-cache add ca-certificates tini
RUN apk add tzdata && \
	cp /usr/share/zoneinfo/Asia/Shanghai /etc/localtime && \
	echo "Asia/Shanghai" > /etc/timezone && \
	apk del tzdata

WORKDIR /root/
ADD aliyundrive-webdav-$TARGETARCH$TARGETVARIANT /usr/bin/aliyundrive-webdav

ENTRYPOINT ["/sbin/tini", "--"]
CMD ["/usr/bin/aliyundrive-webdav", "--host", "0.0.0.0", "--auto-index"]
