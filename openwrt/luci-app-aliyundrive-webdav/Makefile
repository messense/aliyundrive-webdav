include $(TOPDIR)/rules.mk

PKG_NAME:=luci-app-aliyundrive-webdav
PKG_VERSION:=1.7.1
PKG_RELEASE:=1
PKG_PO_VERSION:=$(PKG_VERSION)-$(PKG_RELEASE)

PKG_LICENSE:=MIT
PKG_MAINTAINER:=messense <messense@icloud.com>

LUCI_TITLE:=LuCI Support for aliyundrive-webdav
LUCI_PKGARCH:=all
LUCI_DEPENDS:=+aliyundrive-webdav

include $(TOPDIR)/feeds/luci/luci.mk

# call BuildPackage - OpenWrt buildroot signature
