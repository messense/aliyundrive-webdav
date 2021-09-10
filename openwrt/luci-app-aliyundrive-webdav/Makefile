include $(TOPDIR)/rules.mk

PKG_NAME:=luci-app-aliyundrive-webdav
PKG_VERSION:=0.1.23
PKG_RELEASE:=0

PKG_LICENSE:=MIT

PKG_BUILD_DIR:=$(BUILD_DIR)/$(PKG_NAME)

include $(INCLUDE_DIR)/package.mk

define Package/$(PKG_NAME)
	SECTION:=luci
	CATEGORY:=LuCI
	SUBMENU:=3. Applications
	TITLE:=LuCI Support for aliyundrive-webdav
	PKGARCH:=all
	DEPENDS:=+aliyundrive-webdav
endef

define Package/$(PKG_NAME)/description
LuCI Support for aliyundrive-webdav.
endef

define Build/Configure
endef

define Build/Compile
endef

define Package/$(PKG_NAME)/install
	$(INSTALL_DIR) $(1)/usr/lib/lua/luci/controller
	$(INSTALL_DATA) ./luasrc/controller/*.lua $(1)/usr/lib/lua/luci/controller/
	$(INSTALL_DIR) $(1)/usr/lib/lua/luci/model/cbi/aliyundrive-webdav
	$(INSTALL_DATA) ./luasrc/model/cbi/aliyundrive-webdav/*.lua $(1)/usr/lib/lua/luci/model/cbi/aliyundrive-webdav/
	$(INSTALL_DIR) $(1)/usr/lib/lua/luci/view/aliyundrive-webdav
	$(INSTALL_DATA) ./luasrc/view/aliyundrive-webdav/*.htm $(1)/usr/lib/lua/luci/view/aliyundrive-webdav/
	$(INSTALL_DIR) $(1)/usr/lib/lua/luci/i18n
	po2lmo ./po/zh-cn/aliyundrive-webdav.po $(1)/usr/lib/lua/luci/i18n/aliyundrive-webdav.zh-cn.lmo
endef

$(eval $(call BuildPackage,$(PKG_NAME)))
