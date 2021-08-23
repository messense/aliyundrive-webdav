include $(TOPDIR)/rules.mk

PKG_NAME:=aliyundrive-webdav
PKG_VERSION:=0.1.10
PKG_RELEASE:=0

PKG_LIBC:=musl
ifeq ($(ARCH),arm)
  PKG_LIBC:=musleabi

  ARM_CPU_FEATURES:=$(word 2,$(subst +,$(space),$(call qstrip,$(CONFIG_CPU_TYPE))))
  ifneq ($(filter $(ARM_CPU_FEATURES),vfp vfpv2),)
    PKG_LIBC:=musleabihf
  endif
endif

PKG_SOURCE:=aliyundrive-webdav-$(ARCH)-unknown-linux-$(PKG_LIBC).tar.gz
PKG_SOURCE_URL:=https://github.com/messense/aliyundrive-webdav/releases/download/v$(PKG_VERSION)/
PKG_BUILD_DIR:=$(BUILD_DIR)/$(PKG_NAME)/$(BUILD_VARIANT)/$(PKG_NAME)-$(PKG_VERSION)-$(PKG_RELEASE)
PKG_HASH:=skip

include $(INCLUDE_DIR)/package.mk

define Package/$(PKG_NAME)
	SECTION:=multimedia
	CATEGORY:=Multimedia
	TITLE:=WebDAV server for AliyunDrive
	URL:=https://github.com/messense/aliyundrive-webdav
endef

define Package/$(PKG_NAME)/description
WebDAV server for AliyunDrive.
endef

define Package/$(PKG_NAME)/conffiles
/etc/config/aliyundrive-webdav
endef

define Build/Prepare
	tar -C $(PKG_BUILD_DIR)/ -zxf $(DL_DIR)/$(PKG_SOURCE)
endef

define Build/Compile
	echo "$(PKG_NAME) using precompiled binary."
endef

define Package/$(PKG_NAME)/install
	$(INSTALL_DIR) $(1)/usr/bin
	$(INSTALL_BIN) $(PKG_BUILD_DIR)/aliyundrive-webdav $(1)/usr/bin/
	$(INSTALL_DIR) $(1)/etc/config
	$(INSTALL_CONF) ./root/etc/config/aliyundrive-webdav $(1)/etc/config/aliyundrive-webdav
	$(INSTALL_DIR) $(1)/etc/init.d
	$(INSTALL_BIN) ./root/etc/init.d/aliyundrive-webdav $(1)/etc/init.d/aliyundrive-webdav
endef

$(eval $(call BuildPackage,$(PKG_NAME)))
