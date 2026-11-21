use virtio_drivers::device::gpu::VirtIOGpu;
use virtio_drivers::transport::pci::bus::{Cam, MmioCam};
use virtio_drivers::transport::pci::{bus::PciRoot, PciTransport};
use virtio_drivers::transport::{DeviceType, Transport};
use spin::Mutex;
use noto_sans_mono_bitmap::{get_raster_width, FontWeight, RasterHeight};

use crate::serial_println;
use super::virtio_hal::OsHal;

/// Character cell dimensions for the software console (see `vga::gpu_console`):
/// one glyph cell is `CHAR_WIDTH x CHAR_HEIGHT` pixels, and the framebuffer is
/// sized to exactly fit the existing 80x25 grid — no fractional cells, no
/// runtime resolution negotiation.
pub const CHAR_WIDTH: u32 = get_raster_width(FontWeight::Regular, RasterHeight::Size16) as u32;
pub const CHAR_HEIGHT: u32 = 16;
pub const GFX_WIDTH: u32 = crate::vga::BUFFER_WIDTH as u32 * CHAR_WIDTH;
pub const GFX_HEIGHT: u32 = crate::vga::BUFFER_HEIGHT as u32 * CHAR_HEIGHT;

struct GpuDevice {
    gpu: VirtIOGpu<OsHal, PciTransport>,
    fb_ptr: *mut u8,
    fb_len: usize,
}

// SAFETY: this kernel is single-threaded and poll-based (no interrupts, no
// concurrent access) — see the same reasoning already used for `OsHal`'s
// `unsafe impl Hal`. The raw pointer is only ever touched from `present()`
// below, under this mutex.
unsafe impl Send for GpuDevice {}

static GPU: Mutex<Option<GpuDevice>> = Mutex::new(None);

/// Scan PCI, find the VirtIO GPU device, and return a working driver.
/// Mirrors `scsi::find_and_init_blk`'s bus-walk exactly, filtered on
/// `DeviceType::GPU` instead of `DeviceType::Block`.
pub fn find_and_init_gpu(mmconfig_base: usize) -> Option<VirtIOGpu<OsHal, PciTransport>> {
    let mut pci_root = PciRoot::new(unsafe { MmioCam::new(mmconfig_base as *mut u8, Cam::Ecam) });

    for bus in 0..=255_u8 {
        for (device_function, _device_info) in pci_root.enumerate_bus(bus) {
            use virtio_drivers::transport::pci::bus::Command;
            let (_status, command) = pci_root.get_status_command(device_function);
            pci_root.set_command(device_function, command | Command::BUS_MASTER | Command::MEMORY_SPACE);

            let Ok(transport) = PciTransport::new::<OsHal, _>(&mut pci_root, device_function)
            else {
                continue;
            };

            if transport.device_type() != DeviceType::GPU {
                continue;
            }

            match VirtIOGpu::<OsHal, PciTransport>::new(transport) {
                Ok(gpu) => {
                    serial_println!("[VirtIO] GPU device ready!");
                    return Some(gpu);
                }
                Err(e) => {
                    serial_println!("[VirtIO] Failed to init GPU device: {:?}", e);
                }
            }
        }
    }
    None
}

/// Sets the fixed resolution and stores the driver for `present()` to use.
///
/// Always calls `change_resolution` directly rather than `setup_framebuffer`,
/// since the latter negotiates the host's preferred resolution (often
/// 1024x768+ under plain QEMU virtio-gpu-pci) and would allocate an
/// unpredictable amount from the DMA pool.
pub fn init(mut gpu: VirtIOGpu<OsHal, PciTransport>) -> bool {
    match gpu.change_resolution(GFX_WIDTH, GFX_HEIGHT) {
        Ok(buf) => {
            let fb_ptr = buf.as_mut_ptr();
            let fb_len = buf.len();
            *GPU.lock() = Some(GpuDevice { gpu, fb_ptr, fb_len });
            true
        }
        Err(e) => {
            serial_println!("[gpu] change_resolution failed: {:?}", e);
            false
        }
    }
}

pub fn resolution() -> (u32, u32) {
    (GFX_WIDTH, GFX_HEIGHT)
}

/// Copies a BGRA8888 buffer (exactly `width * height * 4` bytes, matching
/// virtio-gpu's fixed `Format::B8G8R8A8UNORM`) into the real framebuffer and
/// flushes it to the screen. Returns `false` if there's no GPU or `pixels`
/// is the wrong length.
pub fn present(pixels: &[u8]) -> bool {
    let mut guard = GPU.lock();
    let Some(device) = guard.as_mut() else { return false; };

    // The logical framebuffer size (what callers pass) is `w*h*4`, but the
    // underlying DMA allocation (`fb_len`) is rounded up to whole pages by
    // the crate — they only coincide when `w*h*4` happens to be page-aligned.
    // Validate against the logical size and copy into the (never smaller)
    // padded buffer.
    let logical_len = (GFX_WIDTH * GFX_HEIGHT * 4) as usize;
    if pixels.len() != logical_len || logical_len > device.fb_len {
        return false;
    }

    // SAFETY: `fb_ptr`/`fb_len` point into the framebuffer DMA allocation
    // owned by `device.gpu`, which is never reallocated after `init` (we
    // only ever call `change_resolution` once). The only other accessor of
    // this memory is the virtio-gpu device's own DMA read inside `flush()`,
    // which happens strictly after this copy completes — single-threaded,
    // poll-based kernel, so there's no concurrent access to race against.
    unsafe {
        core::slice::from_raw_parts_mut(device.fb_ptr, logical_len).copy_from_slice(pixels);
    }

    device.gpu.flush().is_ok()
}
