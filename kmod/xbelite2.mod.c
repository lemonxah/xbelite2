#include <linux/module.h>
#include <linux/export-internal.h>
#include <linux/compiler.h>

MODULE_INFO(name, KBUILD_MODNAME);

__visible struct module __this_module
__section(".gnu.linkonce.this_module") = {
	.name = KBUILD_MODNAME,
	.init = init_module,
#ifdef CONFIG_MODULE_UNLOAD
	.exit = cleanup_module,
#endif
	.arch = MODULE_ARCH_INIT,
};


MODULE_INFO(depends, "");

MODULE_ALIAS("usb:v045Ep0B00d*dc*dsc*dp*ic*isc*ipD0in*");
MODULE_ALIAS("hid:b0005g*v0000045Ep00000B22");
MODULE_ALIAS("hid:b0005g*v0000045Ep00000B05");

MODULE_INFO(srcversion, "BE4B21D86001590AEEE1118");
