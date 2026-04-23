// SPDX-License-Identifier: GPL-2.0
// Driver registration + kernel API calls.
// Per-report logic lives in xbelite2_logic.c.

#include <linux/module.h>
#include <linux/hid.h>
#include <linux/usb.h>
#include <linux/slab.h>
#include <linux/miscdevice.h>
#include <linux/poll.h>
#include <linux/input.h>

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

// Forward declarations
static struct input_dev *xbelite2_setup_input(struct device *dev, int idx);
static int xbelite2_ff_play(struct input_dev *dev, void *data, struct ff_effect *effect);

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
	struct work_struct init_work;
	struct input_dev *input;
	u8 hw_profile;
	bool profile_synced; /* first 0x0C after connect primes hw_profile */

	/* Force-feedback (rumble) URB and buffer */
	struct urb *irq_out;
	unsigned char *out_buf;
	dma_addr_t out_dma;
	spinlock_t out_lock;
};

static struct xbelite2_usb *g_usb;

// ---- sysfs: hw_profile (read-only, USB) ----
static ssize_t usb_hw_profile_show(struct device *dev,
				   struct device_attribute *attr, char *buf)
{
	struct usb_interface *intf = to_usb_interface(dev);
	struct xbelite2_usb *d = usb_get_intfdata(intf);

	return sysfs_emit(buf, "%u\n", d ? d->hw_profile : 0);
}
static DEVICE_ATTR(hw_profile, 0444, usb_hw_profile_show, NULL);

// ---- BT misc device (ring buffer, like USB) ----
static struct {
	unsigned char ring[RING_SIZE];
	int head, tail;
	spinlock_t lock;
	wait_queue_head_t wait;
	struct miscdevice miscdev;
	bool active;
	struct hid_device *hdev;
	struct input_dev *input;
	u8 hw_profile;
	bool guide_pressed;
	bool menu_pressed;
	bool poweroff_sent;
} g_bt;

// ---- sysfs: hw_profile (read-only, BT) ----
static ssize_t bt_hw_profile_show(struct device *dev,
				  struct device_attribute *attr, char *buf)
{
	return sysfs_emit(buf, "%u\n", g_bt.hw_profile);
}
static struct device_attribute dev_attr_bt_hw_profile =
	__ATTR(hw_profile, 0444, bt_hw_profile_show, NULL);

static void bt_power_off(void);


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
	int ret;
	
	// Prevent duplicate probe - only allow one BT device at a time
	if (g_bt.active) {
		hid_warn(hdev, "BT device already active, rejecting duplicate probe\n");
		return -EBUSY;
	}
	
	ret = hid_parse(hdev);
	if (ret) return ret;
	
	// Claim HID input to prevent hid_microsoft from creating its own input device
	ret = hid_hw_start(hdev, HID_CONNECT_HIDRAW);
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
	g_bt.hw_profile = 0;
	g_bt.guide_pressed = false;
	g_bt.menu_pressed = false;
	g_bt.poweroff_sent = false;
	hid_hw_open(hdev);
	
	g_bt.input = xbelite2_setup_input(&hdev->dev, 0);
	if (!g_bt.input) {
		hid_warn(hdev, "Failed to create input device\n");
		misc_deregister(&g_bt.miscdev);
		hid_hw_close(hdev);
		hid_hw_stop(hdev);
		g_bt.active = false;
		return -ENOMEM;
	}
	
	if (device_create_file(&hdev->dev, &dev_attr_bt_hw_profile))
		hid_warn(hdev, "failed to create hw_profile sysfs attr\n");

	xbelite2_on_bt_connect();
	hid_info(hdev, "Xbox Elite Series 2 connected (BT)\n");
	return 0;
}

static void bt_remove(struct hid_device *hdev)
{
	device_remove_file(&hdev->dev, &dev_attr_bt_hw_profile);

	if (g_bt.input) {
		input_unregister_device(g_bt.input);
		g_bt.input = NULL;
	}
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
	if (g_bt.input && data[0] == 0x01 && size >= 20) {
		u16 lsx = data[1] | (data[2] << 8);
		u16 lsy = data[3] | (data[4] << 8);
		u16 rsx = data[5] | (data[6] << 8);
		u16 rsy = data[7] | (data[8] << 8);
		u16 lt = (data[9] | (data[10] << 8)) & 0x03FF;
		u16 rt = (data[11] | (data[12] << 8)) & 0x03FF;
		u8 hat = data[13] & 0x0F;
		u16 btns = data[14] | (data[15] << 8);
		u8 paddles = size > 19 ? (data[19] & 0x0F) : 0;
		u8 hw_profile = size > 17 ? (data[17] & 0x03) : 0;

		g_bt.hw_profile = hw_profile;

		if (hw_profile != 0) {
			paddles = 0;
		}

		input_report_abs(g_bt.input, ABS_X, (s16)(lsx - 32768));
		input_report_abs(g_bt.input, ABS_Y, (s16)(lsy - 32768));
		input_report_abs(g_bt.input, ABS_RX, (s16)(rsx - 32768));
		input_report_abs(g_bt.input, ABS_RY, (s16)(rsy - 32768));
		input_report_abs(g_bt.input, ABS_Z, lt);
		input_report_abs(g_bt.input, ABS_RZ, rt);

		input_report_key(g_bt.input, BTN_A, btns & (1 << 0));
		input_report_key(g_bt.input, BTN_B, btns & (1 << 1));
		input_report_key(g_bt.input, BTN_X, btns & (1 << 3));
		input_report_key(g_bt.input, BTN_Y, btns & (1 << 4));
		input_report_key(g_bt.input, BTN_TL, btns & (1 << 6));
		input_report_key(g_bt.input, BTN_TR, btns & (1 << 7));
		input_report_key(g_bt.input, BTN_SELECT, btns & (1 << 10));
		
		g_bt.menu_pressed = (btns & (1 << 11)) != 0;
		g_bt.guide_pressed = (btns & (1 << 12)) != 0;
		
		input_report_key(g_bt.input, BTN_START, g_bt.menu_pressed);
		input_report_key(g_bt.input, BTN_MODE, g_bt.guide_pressed);
		input_report_key(g_bt.input, BTN_THUMBL, btns & (1 << 13));
		input_report_key(g_bt.input, BTN_THUMBR, btns & (1 << 14));
		
		if (g_bt.guide_pressed && g_bt.menu_pressed && !g_bt.poweroff_sent) {
			bt_power_off();
			g_bt.poweroff_sent = true;
		} else if (!g_bt.guide_pressed || !g_bt.menu_pressed) {
			g_bt.poweroff_sent = false;
		}

		input_report_abs(g_bt.input, ABS_HAT0X, 
			(hat == 3 || hat == 4 || hat == 2) ? 1 : ((hat == 7 || hat == 8 || hat == 6) ? -1 : 0));
		input_report_abs(g_bt.input, ABS_HAT0Y,
			(hat == 5 || hat == 6 || hat == 4) ? 1 : ((hat == 1 || hat == 8 || hat == 2) ? -1 : 0));

		input_report_key(g_bt.input, BTN_TRIGGER_HAPPY1, paddles & 0x01);
		input_report_key(g_bt.input, BTN_TRIGGER_HAPPY2, paddles & 0x02);
		input_report_key(g_bt.input, BTN_TRIGGER_HAPPY3, paddles & 0x04);
		input_report_key(g_bt.input, BTN_TRIGGER_HAPPY4, paddles & 0x08);

		input_sync(g_bt.input);
	}
	
	bt_ring_push(data, size);
	return xbelite2_on_bt_report(data, size);
}

static void bt_power_off(void)
{
	static const u8 pwr_off[] = {0x05, 0x20, 0x00, 0x01, 0x06};
	int ret;
	
	if (!g_bt.active || !g_bt.hdev) return;
	
	ret = hid_hw_output_report(g_bt.hdev, (u8 *)pwr_off, sizeof(pwr_off));
	if (ret < 0) {
		ret = hid_hw_raw_request(g_bt.hdev, pwr_off[0], (u8 *)pwr_off, 
					 sizeof(pwr_off), HID_OUTPUT_REPORT, HID_REQ_SET_REPORT);
	}
	
	if (ret >= 0) {
		pr_info("xbelite2: BT power-off command sent (Xbox+Menu)\n");
	}
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

// ---- Input device setup ----
static void xbelite2_ff_irq_out(struct urb *urb)
{
	/* URB completion callback - nothing to do */
}

static int xbelite2_ff_play(struct input_dev *dev, void *data, struct ff_effect *effect)
{
	struct xbelite2_usb *usb_priv = input_get_drvdata(dev);
	u16 strong = effect->u.rumble.strong_magnitude;
	u16 weak = effect->u.rumble.weak_magnitude;
	u8 strong_motor = (strong * 100) / 65535;
	u8 weak_motor = (weak * 100) / 65535;
	unsigned long flags;
	int ret;

	if (!usb_priv || !usb_priv->udev || !usb_priv->irq_out)
		return -ENODEV;

	spin_lock_irqsave(&usb_priv->out_lock, flags);
	
	usb_priv->out_buf[0] = 0x09;
	usb_priv->out_buf[1] = 0x00;
	usb_priv->out_buf[2] = 0x00;
	usb_priv->out_buf[3] = 0x09;
	usb_priv->out_buf[4] = 0x00;
	usb_priv->out_buf[5] = 0x0F;
	usb_priv->out_buf[6] = 0;
	usb_priv->out_buf[7] = 0;
	usb_priv->out_buf[8] = strong_motor;
	usb_priv->out_buf[9] = weak_motor;
	usb_priv->out_buf[10] = 0xFF;
	usb_priv->out_buf[11] = 0x00;
	usb_priv->out_buf[12] = 0xEB;

	usb_priv->irq_out->transfer_buffer_length = 13;
	
	ret = usb_submit_urb(usb_priv->irq_out, GFP_ATOMIC);
	
	spin_unlock_irqrestore(&usb_priv->out_lock, flags);

	if (ret < 0)
		pr_err("xbelite2_ff_play: usb_submit_urb failed: %d\n", ret);

	return ret;
}

static struct input_dev *xbelite2_setup_input(struct device *dev, int idx)
{
	struct input_dev *input;
	int ret;

	input = input_allocate_device();
	if (!input)
		return NULL;

	input->name = "Xbox Elite Wireless Controller Series 2";
	input->id.bustype = BUS_USB;
	input->id.vendor = VENDOR_MS;
	input->id.product = PID_USB;
	input->id.version = 0x0100;
	input->dev.parent = dev;

	input_set_capability(input, EV_KEY, BTN_A);
	input_set_capability(input, EV_KEY, BTN_B);
	input_set_capability(input, EV_KEY, BTN_X);
	input_set_capability(input, EV_KEY, BTN_Y);
	input_set_capability(input, EV_KEY, BTN_TL);
	input_set_capability(input, EV_KEY, BTN_TR);
	input_set_capability(input, EV_KEY, BTN_SELECT);
	input_set_capability(input, EV_KEY, BTN_START);
	input_set_capability(input, EV_KEY, BTN_MODE);
	input_set_capability(input, EV_KEY, BTN_THUMBL);
	input_set_capability(input, EV_KEY, BTN_THUMBR);
	input_set_capability(input, EV_KEY, BTN_TRIGGER_HAPPY1);
	input_set_capability(input, EV_KEY, BTN_TRIGGER_HAPPY2);
	input_set_capability(input, EV_KEY, BTN_TRIGGER_HAPPY3);
	input_set_capability(input, EV_KEY, BTN_TRIGGER_HAPPY4);

	input_set_abs_params(input, ABS_X, -32768, 32767, 16, 128);
	input_set_abs_params(input, ABS_Y, -32768, 32767, 16, 128);
	input_set_abs_params(input, ABS_RX, -32768, 32767, 16, 128);
	input_set_abs_params(input, ABS_RY, -32768, 32767, 16, 128);
	input_set_abs_params(input, ABS_Z, 0, 1023, 0, 0);
	input_set_abs_params(input, ABS_RZ, 0, 1023, 0, 0);
	input_set_abs_params(input, ABS_HAT0X, -1, 1, 0, 0);
	input_set_abs_params(input, ABS_HAT0Y, -1, 1, 0, 0);

	input_set_capability(input, EV_FF, FF_RUMBLE);
	ret = input_ff_create_memless(input, NULL, xbelite2_ff_play);
	if (ret) {
		input_free_device(input);
		return NULL;
	}

	ret = input_register_device(input);
	if (ret) {
		pr_err("xbelite2: input_register_device failed: %d\n", ret);
		input_free_device(input);
		return NULL;
	}
	
	pr_info("xbelite2: Input device registered successfully\n");

	return input;
}

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

static void misc_write_cb(struct urb *urb)
{
	complete((struct completion *)urb->context);
}

static ssize_t misc_write(struct file *f, const char __user *buf,
			  size_t cnt, loff_t *p)
{
	struct xbelite2_usb *d = g_usb;
	struct urb *urb;
	unsigned char *kbuf;
	DECLARE_COMPLETION_ONSTACK(done);
	int ret;

	if (!d || !d->udev) return -ENODEV;
	if (cnt > 64 || cnt == 0) return -EINVAL;

	kbuf = kmalloc(cnt, GFP_KERNEL);
	if (!kbuf) return -ENOMEM;
	if (copy_from_user(kbuf, buf, cnt)) { kfree(kbuf); return -EFAULT; }

	urb = usb_alloc_urb(0, GFP_KERNEL);
	if (!urb) { kfree(kbuf); return -ENOMEM; }

	usb_fill_int_urb(urb, d->udev,
			 usb_sndintpipe(d->udev, 0x02),
			 kbuf, cnt, misc_write_cb, &done, 4);

	ret = usb_submit_urb(urb, GFP_KERNEL);
	if (ret) {
		usb_free_urb(urb);
		kfree(kbuf);
		return -EIO;
	}

	if (!wait_for_completion_timeout(&done, msecs_to_jiffies(2000))) {
		usb_kill_urb(urb);
		usb_free_urb(urb);
		kfree(kbuf);
		return -ETIMEDOUT;
	}

	ret = urb->status;
	usb_free_urb(urb);
	kfree(kbuf);
	return ret ? ret : cnt;
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

// ---- USB GIP init work (runs in process context, safe to sleep) ----
// Init sequence matches Linux kernel xpad.c for Elite 2 (0x045e:0x0b00)
static void usb_init_work(struct work_struct *work)
{
	struct xbelite2_usb *d = container_of(work, struct xbelite2_usb, init_work);
	// 1. Power on (all Xbox One controllers)
	static const u8 pwr[] = {0x05, 0x20, 0x00, 0x01, 0x00};
	// 2. BT→USB transition init (required for Elite 2 / Xbox One S)
	static const u8 s_init[] = {0x05, 0x20, 0x00, 0x0f, 0x06};
	// 3. Extended reports (paddles, profile data)
	static const u8 ext_init[] = {0x4D, 0x10, 0x01, 0x02, 0x07, 0x00};
	// 4. LED on
	static const u8 led_on[] = {0x0A, 0x20, 0x00, 0x03, 0x00, 0x01, 0x14};
	// 5. Auth done
	static const u8 auth_done[] = {0x06, 0x20, 0x00, 0x02, 0x01, 0x00};
	int a;

	if (!d->running) return;
	usb_interrupt_msg(d->udev, usb_sndintpipe(d->udev, 0x02),
			  (void *)pwr, sizeof(pwr), &a, 1000);
	usb_interrupt_msg(d->udev, usb_sndintpipe(d->udev, 0x02),
			  (void *)s_init, sizeof(s_init), &a, 1000);
	usb_interrupt_msg(d->udev, usb_sndintpipe(d->udev, 0x02),
			  (void *)ext_init, sizeof(ext_init), &a, 1000);
	usb_interrupt_msg(d->udev, usb_sndintpipe(d->udev, 0x02),
			  (void *)led_on, sizeof(led_on), &a, 1000);
	usb_interrupt_msg(d->udev, usb_sndintpipe(d->udev, 0x02),
			  (void *)auth_done, sizeof(auth_done), &a, 1000);
}

// ---- USB GIP IRQ ----
static void usb_irq(struct urb *urb)
{
	struct xbelite2_usb *d = urb->context;
	unsigned char *data = d->in_buf;
	int len = urb->actual_length;
	struct input_dev *input = d->input;

	if (urb->status) goto resubmit;

	if (len >= 4) {
		if (data[0] == 0x02 && !d->init_sent) { // HELLO
			d->init_sent = true;
			schedule_work(&d->init_work);
		}
		if (xbelite2_on_gip_message(data, len))
			ring_push(d, data, len);

		// Input event reporting
		if (input) {
			// Guide button (0x07) - payload byte 0 (data[4])
			if (data[0] == 0x07 && len >= 5) {
				input_report_key(input, BTN_MODE, data[4] != 0);
				input_sync(input);
			}

			// Profile switch (0x1E sub 0x03) - track hw_profile
			if (data[0] == 0x1E && len >= 6 && data[4] == 0x03) {
				d->hw_profile = data[5];
			}

			// Elite extended report (0x0C) - profile at payload byte 15 (data[19])
			if (data[0] == 0x0C && len >= 20) {
				u8 new_profile = data[19];
				if (!d->profile_synced) {
					// First post-connect report: trust it unconditionally
					// so sysfs reflects the controller's active profile.
					d->hw_profile = new_profile;
					d->profile_synced = true;
				} else {
					// Filter idle glitches where profile byte reads 0.
					int has_input = 0;
					int i;
					for (i = 4; i < 19; i++) {
						if (data[i] != 0) {
							has_input = 1;
							break;
						}
					}
					if (new_profile != d->hw_profile &&
					    (new_profile != 0 || has_input)) {
						d->hw_profile = new_profile;
					}
				}
			}

			// Input report (0x20) - GIP input data
			if (data[0] == 0x20 && len >= 18) {
				u8 b1 = data[4];  // Buttons byte 1
				u8 b2 = data[5];  // Buttons byte 2
				s16 lx, ly, rx, ry;
				u16 lt, rt;
				u8 paddles = 0;

				// Buttons — use BTN_A/B/X/Y to match BT path and xpad convention.
				// Note: in Linux headers BTN_X == BTN_NORTH and BTN_Y == BTN_WEST;
				// reporting via BTN_X/BTN_Y keeps Xbox labels consistent across
				// USB and BT connections (games that read raw evdev see the same
				// keycodes either way).
				input_report_key(input, BTN_START, b1 & (1 << 2));  // Menu
				input_report_key(input, BTN_SELECT, b1 & (1 << 3)); // View
				input_report_key(input, BTN_A, b1 & (1 << 4));      // A
				input_report_key(input, BTN_B, b1 & (1 << 5));      // B
				input_report_key(input, BTN_X, b1 & (1 << 6));      // X
				input_report_key(input, BTN_Y, b1 & (1 << 7));      // Y

				input_report_key(input, BTN_TL, b2 & (1 << 4));     // LB
				input_report_key(input, BTN_TR, b2 & (1 << 5));     // RB
				input_report_key(input, BTN_THUMBL, b2 & (1 << 6)); // LS
				input_report_key(input, BTN_THUMBR, b2 & (1 << 7)); // RS

				// D-pad
				input_report_abs(input, ABS_HAT0X, 
					(b2 & (1 << 3)) ? 1 : ((b2 & (1 << 2)) ? -1 : 0)); // Right:Left
				input_report_abs(input, ABS_HAT0Y,
					(b2 & (1 << 1)) ? 1 : ((b2 & (1 << 0)) ? -1 : 0)); // Down:Up

				// Triggers (u16 LE, 0-1023)
				lt = (u16)data[6] | ((u16)data[7] << 8);
				rt = (u16)data[8] | ((u16)data[9] << 8);
				input_report_abs(input, ABS_Z, lt);
				input_report_abs(input, ABS_RZ, rt);

				// Sticks (i16 LE) - Y axes inverted in GIP
				lx = (s16)((u16)data[10] | ((u16)data[11] << 8));
				ly = (s16)((u16)data[12] | ((u16)data[13] << 8));
				rx = (s16)((u16)data[14] | ((u16)data[15] << 8));
				ry = (s16)((u16)data[16] | ((u16)data[17] << 8));

				input_report_abs(input, ABS_X, lx);
				input_report_abs(input, ABS_Y, ~ly);  // Invert Y
				input_report_abs(input, ABS_RX, rx);
				input_report_abs(input, ABS_RY, ~ry); // Invert Y

				// Paddles (byte 18) - suppress on hw_profile != 0
				if (len > 18) {
					paddles = data[18] & 0x0F;
					if (d->hw_profile != 0) {
						paddles = 0; // Suppress on profiles 1-3
					}
				}
				input_report_key(input, BTN_TRIGGER_HAPPY1, paddles & 0x01); // P1
				input_report_key(input, BTN_TRIGGER_HAPPY2, paddles & 0x02); // P2
				input_report_key(input, BTN_TRIGGER_HAPPY3, paddles & 0x04); // P3
				input_report_key(input, BTN_TRIGGER_HAPPY4, paddles & 0x08); // P4

				input_sync(input);
			}
		}
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
	
	// Prevent duplicate probe - only allow one USB device at a time
	if (g_usb) {
		dev_warn(&intf->dev, "USB device already active, rejecting duplicate probe\n");
		return -EBUSY;
	}
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

	d->out_buf = usb_alloc_coherent(udev, 64, GFP_KERNEL, &d->out_dma);
	if (!d->out_buf) { ret = -ENOMEM; goto err2_5; }
	d->irq_out = usb_alloc_urb(0, GFP_KERNEL);
	if (!d->irq_out) { ret = -ENOMEM; goto err2_6; }
	
	spin_lock_init(&d->out_lock);
	usb_fill_int_urb(d->irq_out, udev,
		usb_sndintpipe(udev, 0x02),
		d->out_buf, 64, xbelite2_ff_irq_out, d, 4);
	d->irq_out->transfer_dma = d->out_dma;
	d->irq_out->transfer_flags |= URB_NO_TRANSFER_DMA_MAP;

	d->miscdev.minor = MISC_DYNAMIC_MINOR;
	d->miscdev.name = "xbelite2";
	d->miscdev.fops = &misc_fops;
	d->miscdev.mode = 0600;
	ret = misc_register(&d->miscdev);
	if (ret) goto err3;

	INIT_WORK(&d->init_work, usb_init_work);
	d->running = true;
	d->hw_profile = 0;
	d->profile_synced = false;
	g_usb = d;
	ret = usb_submit_urb(d->irq_in, GFP_KERNEL);
	if (ret) goto err4;

	dev_info(&intf->dev, "Creating input device...\n");
	d->input = xbelite2_setup_input(&intf->dev, 1);
	if (!d->input) {
		dev_err(&intf->dev, "Failed to create input device\n");
		usb_kill_urb(d->irq_in);
		ret = -ENOMEM;
		goto err4;
	}
	dev_info(&intf->dev, "Input device created, setting drvdata\n");
	input_set_drvdata(d->input, d);

	usb_set_intfdata(intf, d);

	if (device_create_file(&intf->dev, &dev_attr_hw_profile))
		dev_warn(&intf->dev, "failed to create hw_profile sysfs attr\n");

	xbelite2_on_usb_connect();
	dev_info(&intf->dev, "Xbox Elite Series 2 connected (USB)\n");
	return 0;

err4: misc_deregister(&d->miscdev);
err3: usb_free_urb(d->irq_out);
err2_6: usb_free_coherent(udev, 64, d->out_buf, d->out_dma);
err2_5: usb_free_urb(d->irq_in);
err2: usb_free_coherent(udev, d->in_size, d->in_buf, d->in_dma);
err1: usb_put_dev(udev); kfree(d); return ret;
}

static void usb_disconnect(struct usb_interface *intf)
{
	struct xbelite2_usb *d = usb_get_intfdata(intf);
	int a;
	static const u8 usb_to_bt[] = {0x05, 0x20, 0x00, 0x0f, 0x00};
	
	if (!d) return;

	device_remove_file(&intf->dev, &dev_attr_hw_profile);

	if (d->input) {
		input_unregister_device(d->input);
		d->input = NULL;
	}
	
	if (d->running && d->udev) {
		usb_interrupt_msg(d->udev, usb_sndintpipe(d->udev, 0x02),
				  (void *)usb_to_bt, sizeof(usb_to_bt), &a, 1000);
	}
	
	d->running = false;
	g_usb = NULL;
	xbelite2_on_usb_disconnect();
	wake_up_interruptible(&d->ring_wait);
	usb_kill_urb(d->irq_in);
	usb_kill_urb(d->irq_out);
	cancel_work_sync(&d->init_work);
	misc_deregister(&d->miscdev);
	usb_free_urb(d->irq_in);
	usb_free_urb(d->irq_out);
	usb_free_coherent(d->udev, d->in_size, d->in_buf, d->in_dma);
	usb_free_coherent(d->udev, 64, d->out_buf, d->out_dma);
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
MODULE_SOFTDEP("pre: ff_memless");
