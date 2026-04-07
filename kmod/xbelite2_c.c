// SPDX-License-Identifier: GPL-2.0
// C shim — driver registration + kernel API calls.
// Logic lives in xbelite2_rust.rs.

#include <linux/module.h>
#include <linux/hid.h>
#include <linux/usb.h>
#include <linux/slab.h>
#include <linux/miscdevice.h>
#include <linux/poll.h>

#define VENDOR_MS    0x045E
#define PID_USB      0x0B00
#define PID_BLE      0x0B22
#define PID_BT       0x0B05
#define RING_SIZE    4096

// Rust callbacks
extern void xbelite2_on_bt_connect(void);
extern void xbelite2_on_bt_disconnect(void);
extern int xbelite2_on_bt_report(const u8 *data, int size);
extern void xbelite2_on_usb_connect(void);
extern void xbelite2_on_usb_disconnect(void);
extern int xbelite2_on_gip_message(const u8 *data, int size);

// ---- USB GIP state ----
struct xbelite2_usb {
	struct usb_device *udev;
	struct urb *irq_in;
	unsigned char *in_buf;
	dma_addr_t in_dma;
	int in_size;
	bool running;
	bool init_sent;
	unsigned char ring[RING_SIZE];
	int ring_head, ring_tail;
	spinlock_t ring_lock;
	wait_queue_head_t ring_wait;
	struct miscdevice miscdev;
};

static struct xbelite2_usb *g_usb;

// ---- BT misc device (ring buffer, like USB) ----
static struct {
	unsigned char ring[RING_SIZE];
	int head, tail;
	spinlock_t lock;
	wait_queue_head_t wait;
	struct miscdevice miscdev;
	bool active;
	struct hid_device *hdev;
} g_bt;

static void bt_ring_push(const u8 *data, int len)
{
	unsigned long flags;
	int i;
	spin_lock_irqsave(&g_bt.lock, flags);
	g_bt.ring[g_bt.head] = len & 0xFF;
	g_bt.head = (g_bt.head + 1) & (RING_SIZE - 1);
	g_bt.ring[g_bt.head] = (len >> 8) & 0xFF;
	g_bt.head = (g_bt.head + 1) & (RING_SIZE - 1);
	for (i = 0; i < len; i++) {
		g_bt.ring[g_bt.head] = data[i];
		g_bt.head = (g_bt.head + 1) & (RING_SIZE - 1);
	}
	spin_unlock_irqrestore(&g_bt.lock, flags);
	wake_up_interruptible(&g_bt.wait);
}

static ssize_t bt_misc_read(struct file *f, char __user *buf, size_t cnt, loff_t *p)
{
	unsigned long flags;
	ssize_t copied = 0;
	if (!g_bt.active) return -ENODEV;
	if (g_bt.head == g_bt.tail) {
		if (f->f_flags & O_NONBLOCK) return -EAGAIN;
		if (wait_event_interruptible(g_bt.wait,
		    g_bt.head != g_bt.tail || !g_bt.active))
			return -ERESTARTSYS;
	}
	if (!g_bt.active) return -ENODEV;
	spin_lock_irqsave(&g_bt.lock, flags);
	while (copied < cnt && g_bt.tail != g_bt.head) {
		u8 b = g_bt.ring[g_bt.tail];
		g_bt.tail = (g_bt.tail + 1) & (RING_SIZE - 1);
		spin_unlock_irqrestore(&g_bt.lock, flags);
		if (put_user(b, buf + copied)) return -EFAULT;
		copied++;
		spin_lock_irqsave(&g_bt.lock, flags);
	}
	spin_unlock_irqrestore(&g_bt.lock, flags);
	return copied;
}

static ssize_t bt_misc_write(struct file *f, const char __user *buf,
			     size_t cnt, loff_t *p)
{
	unsigned char kbuf[64];
	struct hid_device *hdev;
	int ret;
	if (!g_bt.active) return -ENODEV;
	hdev = g_bt.hdev;
	if (!hdev) return -ENODEV;
	if (cnt < 1 || cnt > sizeof(kbuf)) return -EINVAL;
	if (copy_from_user(kbuf, buf, cnt)) return -EFAULT;

	/* Try output_report first (works for USB-style transports),
	   fall back to raw_request SET_REPORT for Bluetooth HID. */
	ret = hid_hw_output_report(hdev, kbuf, cnt);
	if (ret < 0)
		ret = hid_hw_raw_request(hdev, kbuf[0], kbuf, cnt,
					 HID_OUTPUT_REPORT, HID_REQ_SET_REPORT);
	if (ret < 0)
		return -EIO;
	return cnt;
}

static __poll_t bt_misc_poll(struct file *f, struct poll_table_struct *w)
{
	if (!g_bt.active) return POLLERR;
	poll_wait(f, &g_bt.wait, w);
	return (g_bt.head != g_bt.tail) ? (POLLIN | POLLRDNORM) : 0;
}

static const struct file_operations bt_misc_fops = {
	.owner = THIS_MODULE, .read = bt_misc_read,
	.write = bt_misc_write, .poll = bt_misc_poll,
};

// ---- BT HID ----
static int bt_probe(struct hid_device *hdev, const struct hid_device_id *id)
{
	int ret = hid_parse(hdev);
	if (ret) return ret;
	ret = hid_hw_start(hdev, HID_CONNECT_DRIVER);
	if (ret) return ret;

	spin_lock_init(&g_bt.lock);
	init_waitqueue_head(&g_bt.wait);
	g_bt.head = 0;
	g_bt.tail = 0;
	g_bt.hdev = hdev;

	g_bt.miscdev.minor = MISC_DYNAMIC_MINOR;
	g_bt.miscdev.name = "xbelite2_bt";
	g_bt.miscdev.fops = &bt_misc_fops;
	g_bt.miscdev.mode = 0600;
	ret = misc_register(&g_bt.miscdev);
	if (ret) {
		hid_hw_stop(hdev);
		return ret;
	}

	g_bt.active = true;
	hid_hw_open(hdev);
	xbelite2_on_bt_connect();
	hid_info(hdev, "Xbox Elite Series 2 connected (BT)\n");
	return 0;
}

static void bt_remove(struct hid_device *hdev)
{
	g_bt.active = false;
	g_bt.hdev = NULL;
	xbelite2_on_bt_disconnect();
	wake_up_interruptible(&g_bt.wait);
	hid_hw_close(hdev);
	misc_deregister(&g_bt.miscdev);
	hid_hw_stop(hdev);
}

static int bt_raw_event(struct hid_device *hdev, struct hid_report *report,
			u8 *data, int size)
{
	bt_ring_push(data, size);
	return xbelite2_on_bt_report(data, size);
}

static const struct hid_device_id bt_ids[] = {
	{ HID_BLUETOOTH_DEVICE(VENDOR_MS, PID_BLE) },
	{ HID_BLUETOOTH_DEVICE(VENDOR_MS, PID_BT) },
	{ }
};
MODULE_DEVICE_TABLE(hid, bt_ids);

static struct hid_driver bt_driver = {
	.name      = "xbelite2",
	.id_table  = bt_ids,
	.probe     = bt_probe,
	.remove    = bt_remove,
	.raw_event = bt_raw_event,
};

// ---- USB GIP ring buffer + misc device ----
static void ring_push(struct xbelite2_usb *d, const u8 *data, int len)
{
	unsigned long flags;
	int i;
	spin_lock_irqsave(&d->ring_lock, flags);
	d->ring[d->ring_head] = len & 0xFF;
	d->ring_head = (d->ring_head + 1) & (RING_SIZE - 1);
	d->ring[d->ring_head] = (len >> 8) & 0xFF;
	d->ring_head = (d->ring_head + 1) & (RING_SIZE - 1);
	for (i = 0; i < len; i++) {
		d->ring[d->ring_head] = data[i];
		d->ring_head = (d->ring_head + 1) & (RING_SIZE - 1);
	}
	spin_unlock_irqrestore(&d->ring_lock, flags);
	wake_up_interruptible(&d->ring_wait);
}

static ssize_t misc_read(struct file *f, char __user *buf, size_t cnt, loff_t *p)
{
	struct xbelite2_usb *d = g_usb;
	unsigned long flags;
	ssize_t copied = 0;
	if (!d) return -ENODEV;
	if (d->ring_head == d->ring_tail) {
		if (f->f_flags & O_NONBLOCK) return -EAGAIN;
		if (wait_event_interruptible(d->ring_wait,
		    d->ring_head != d->ring_tail || !d->running))
			return -ERESTARTSYS;
	}
	spin_lock_irqsave(&d->ring_lock, flags);
	while (copied < cnt && d->ring_tail != d->ring_head) {
		u8 b = d->ring[d->ring_tail];
		d->ring_tail = (d->ring_tail + 1) & (RING_SIZE - 1);
		spin_unlock_irqrestore(&d->ring_lock, flags);
		if (put_user(b, buf + copied)) return -EFAULT;
		copied++;
		spin_lock_irqsave(&d->ring_lock, flags);
	}
	spin_unlock_irqrestore(&d->ring_lock, flags);
	return copied;
}

static ssize_t misc_write(struct file *f, const char __user *buf,
			  size_t cnt, loff_t *p)
{
	struct xbelite2_usb *d = g_usb;
	unsigned char kbuf[64];
	int actual;
	if (!d || !d->udev) return -ENODEV;
	if (cnt > sizeof(kbuf)) return -EINVAL;
	if (copy_from_user(kbuf, buf, cnt)) return -EFAULT;
	if (usb_interrupt_msg(d->udev, usb_sndintpipe(d->udev, 0x02),
			      kbuf, cnt, &actual, 1000))
		return -EIO;
	return actual;
}

static __poll_t misc_poll(struct file *f, struct poll_table_struct *w)
{
	struct xbelite2_usb *d = g_usb;
	if (!d) return POLLERR;
	poll_wait(f, &d->ring_wait, w);
	return (d->ring_head != d->ring_tail) ? (POLLIN | POLLRDNORM) : 0;
}

static const struct file_operations misc_fops = {
	.owner = THIS_MODULE, .read = misc_read,
	.write = misc_write, .poll = misc_poll,
};

// ---- USB GIP IRQ ----
static void usb_irq(struct urb *urb)
{
	struct xbelite2_usb *d = urb->context;
	unsigned char *data = d->in_buf;
	int len = urb->actual_length;

	if (urb->status) goto resubmit;

	if (len >= 4) {
		if (data[0] == 0x02 && !d->init_sent) { // HELLO
			static const u8 pwr[] = {0x05, 0x20, 0x00, 0x01, 0x00};
			static const u8 init[] = {0x4D, 0x10, 0x01, 0x02, 0x07, 0x00};
			int a;
			usb_interrupt_msg(d->udev, usb_sndintpipe(d->udev, 0x02),
					  (void *)pwr, sizeof(pwr), &a, 100);
			usb_interrupt_msg(d->udev, usb_sndintpipe(d->udev, 0x02),
					  (void *)init, sizeof(init), &a, 100);
			d->init_sent = true;
		}
		if (xbelite2_on_gip_message(data, len))
			ring_push(d, data, len);
	}
resubmit:
	if (d->running)
		usb_submit_urb(d->irq_in, GFP_ATOMIC);
}

// ---- USB probe/disconnect ----
static int usb_probe(struct usb_interface *intf, const struct usb_device_id *id)
{
	struct usb_device *udev = interface_to_usbdev(intf);
	struct usb_endpoint_descriptor *ep = NULL;
	struct xbelite2_usb *d;
	int i, ret;

	if (intf->cur_altsetting->desc.bInterfaceNumber != 0)
		return -ENODEV;
	for (i = 0; i < intf->cur_altsetting->desc.bNumEndpoints; i++) {
		struct usb_endpoint_descriptor *e =
			&intf->cur_altsetting->endpoint[i].desc;
		if (usb_endpoint_is_int_in(e)) { ep = e; break; }
	}
	if (!ep) return -ENODEV;

	d = kzalloc(sizeof(*d), GFP_KERNEL);
	if (!d) return -ENOMEM;
	d->udev = usb_get_dev(udev);
	d->in_size = le16_to_cpu(ep->wMaxPacketSize);
	spin_lock_init(&d->ring_lock);
	init_waitqueue_head(&d->ring_wait);

	d->in_buf = usb_alloc_coherent(udev, d->in_size, GFP_KERNEL, &d->in_dma);
	if (!d->in_buf) { ret = -ENOMEM; goto err1; }
	d->irq_in = usb_alloc_urb(0, GFP_KERNEL);
	if (!d->irq_in) { ret = -ENOMEM; goto err2; }

	usb_fill_int_urb(d->irq_in, udev,
		usb_rcvintpipe(udev, ep->bEndpointAddress),
		d->in_buf, d->in_size, usb_irq, d, ep->bInterval);
	d->irq_in->transfer_dma = d->in_dma;
	d->irq_in->transfer_flags |= URB_NO_TRANSFER_DMA_MAP;

	d->miscdev.minor = MISC_DYNAMIC_MINOR;
	d->miscdev.name = "xbelite2";
	d->miscdev.fops = &misc_fops;
	d->miscdev.mode = 0600;
	ret = misc_register(&d->miscdev);
	if (ret) goto err3;

	d->running = true;
	g_usb = d;
	ret = usb_submit_urb(d->irq_in, GFP_KERNEL);
	if (ret) goto err4;

	usb_set_intfdata(intf, d);
	xbelite2_on_usb_connect();
	dev_info(&intf->dev, "Xbox Elite Series 2 connected (USB)\n");
	return 0;

err4: misc_deregister(&d->miscdev);
err3: usb_free_urb(d->irq_in);
err2: usb_free_coherent(udev, d->in_size, d->in_buf, d->in_dma);
err1: usb_put_dev(udev); kfree(d); return ret;
}

static void usb_disconnect(struct usb_interface *intf)
{
	struct xbelite2_usb *d = usb_get_intfdata(intf);
	if (!d) return;
	d->running = false;
	g_usb = NULL;
	xbelite2_on_usb_disconnect();
	wake_up_interruptible(&d->ring_wait);
	usb_kill_urb(d->irq_in);
	misc_deregister(&d->miscdev);
	usb_free_urb(d->irq_in);
	usb_free_coherent(d->udev, d->in_size, d->in_buf, d->in_dma);
	usb_put_dev(d->udev);
	usb_set_intfdata(intf, NULL);
	kfree(d);
	pr_info("xbelite2: disconnected (USB)\n");
}

static const struct usb_device_id usb_ids[] = {
	{ USB_DEVICE_INTERFACE_PROTOCOL(VENDOR_MS, PID_USB, 0xD0) },
	{ }
};
MODULE_DEVICE_TABLE(usb, usb_ids);

static struct usb_driver usb_drv = {
	.name = "xbelite2", .id_table = usb_ids,
	.probe = usb_probe, .disconnect = usb_disconnect,
};

// ---- Module ----
static int __init xbelite2_init(void)
{
	int ret;
	ret = hid_register_driver(&bt_driver);
	if (ret) return ret;
	ret = usb_register(&usb_drv);
	if (ret) { hid_unregister_driver(&bt_driver); return ret; }
	pr_info("xbelite2: loaded (BT + USB)\n");
	return 0;
}

static void __exit xbelite2_exit(void)
{
	usb_deregister(&usb_drv);
	hid_unregister_driver(&bt_driver);
	pr_info("xbelite2: unloaded\n");
}

module_init(xbelite2_init);
module_exit(xbelite2_exit);
MODULE_LICENSE("GPL");
MODULE_DESCRIPTION("Xbox Elite Series 2 Controller driver");
